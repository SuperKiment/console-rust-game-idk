//! World persistence: one postcard file, written atomically.

use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

use idk_shared::content::{Species, Tile};
use idk_shared::geo::{CHUNK_AREA, ChunkPos, Dir, Pos};
use idk_shared::protocol::EntityId;
use serde::{Deserialize, Serialize};

use crate::game::Game;
use crate::world::World;

/// Version courante du format. Ajouter une version (3 étapes) :
/// 1. copier la struct `SaveData` actuelle, figée, dans un module `v<N>` (N = ancienne version) ;
/// 2. incrémenter `SAVE_VERSION` puis modifier `SaveData` ;
/// 3. ajouter dans [`decode`] le bras `N => v<N>::SaveData décodé puis converti en SaveData`.
///
/// `version` doit rester le premier champ de `SaveData` : [`decode`] le lit seul en tête.
pub const SAVE_VERSION: u32 = 1;
const FILE: &str = "world.bin";

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub version: u32,
    pub seed: u64,
    pub next_id: EntityId,
    pub players: Vec<(String, Pos, Dir)>,
    pub chunks: Vec<(ChunkPos, Vec<Tile>)>,
    pub spawned: Vec<ChunkPos>,
    pub animals: Vec<(Species, Pos, Option<String>)>,
}

impl Game {
    pub fn to_save(&self) -> SaveData {
        let online = self.players.values().map(|p| (p.name.clone(), p.pos, p.facing));
        let offline = self.offline.iter().map(|(n, (p, d))| (n.clone(), *p, *d));
        SaveData {
            version: SAVE_VERSION,
            seed: self.world.seed,
            next_id: self.next_id,
            players: online.chain(offline).collect(),
            chunks: self.world.modified_chunks().map(|(p, t)| (p, t.clone())).collect(),
            spawned: self.world.spawned().collect(),
            animals: self.animals().values().map(|a| (a.species, a.pos(), a.tamed_by.clone())).collect(),
        }
    }

    pub fn from_save(s: SaveData, now: Instant) -> Game {
        let mut g = Game::new(World::restore(s.seed, s.chunks, s.spawned));
        g.next_id = s.next_id;
        g.offline = s.players.into_iter().map(|(n, p, d)| (n, (p, d))).collect();
        for (species, pos, tamed_by) in s.animals {
            // Deux animaux sur la même tuile (vieille save) : le second est ignoré.
            if g.insert_animal(species, pos, tamed_by, now).is_none() {
                tracing::warn!(?pos, "save has two animals on one tile, one dropped");
            }
        }
        g
    }
}

pub fn read(dir: &Path) -> io::Result<Option<SaveData>> {
    let path = dir.join(FILE);
    let bytes = match std::fs::read(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        r => r?,
    };
    // Le chemin dans le message : le serveur refuse de démarrer, l'utilisateur doit savoir quel fichier.
    decode(&bytes).map(Some).map_err(|e| io::Error::other(format!("{}: {e}", path.display())))
}

/// Lit `version` seule (premier champ, varint) avant de décoder le reste : un ancien format
/// dont la structure diffère n'échoue donc pas au décodage de la struct courante.
fn decode(bytes: &[u8]) -> io::Result<SaveData> {
    let (version, _) = postcard::take_from_bytes::<u32>(bytes).map_err(io::Error::other)?;
    match version {
        SAVE_VERSION => {
            let data: SaveData = postcard::from_bytes(bytes).map_err(io::Error::other)?;
            validate(&data)?;
            Ok(data)
        }
        v => Err(io::Error::other(format!("unsupported save version {v}"))),
    }
}

/// Refuse une save dont un chunk n'a pas `CHUNK_AREA` tuiles (sinon panic d'index au 1er accès).
/// Un id de tuile inconnu est déjà refusé par le décodage postcard (variante d'enum invalide).
fn validate(data: &SaveData) -> io::Result<()> {
    for (cp, tiles) in &data.chunks {
        if tiles.len() != CHUNK_AREA {
            return Err(io::Error::other(format!("invalid chunk at ({}, {})", cp.x, cp.y)));
        }
    }
    Ok(())
}

pub fn write(dir: &Path, data: &SaveData) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let bytes = postcard::to_stdvec(data).map_err(io::Error::other)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    // fsync du tmp, puis du dossier après le rename : une coupure de courant ne laisse
    // jamais un `world.bin` vide ou tronqué (soit l'ancien, soit le nouveau).
    let mut file = std::fs::File::create(&tmp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    std::fs::rename(tmp, dir.join(FILE))?;
    #[cfg(unix)]
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SaveData {
        SaveData {
            version: SAVE_VERSION,
            seed: 42,
            next_id: 7,
            players: vec![("bob".into(), Pos::new(1, 2), Dir::North)],
            chunks: vec![],
            spawned: vec![],
            animals: vec![],
        }
    }

    #[test]
    fn loaded_save_has_consistent_animal_index() {
        let now = Instant::now();
        let mut data = sample();
        let (a, b) = (Pos::new(3, 4), Pos::new(5, 6));
        data.animals = vec![
            (Species::Cow, a, None),
            (Species::Dog, b, Some("bob".into())),
            (Species::Rabbit, a, None), // doublon : ignoré
        ];
        let g = Game::from_save(data, now);
        assert_eq!(g.animals().len(), 2);
        assert!(g.animal_at(a).is_some() && g.animal_at(b).is_some());
        g.assert_animal_index();
    }

    #[test]
    fn roundtrip_current_version() {
        let dir = std::env::temp_dir().join(format!("idk-save-test-{}", std::process::id()));
        write(&dir, &sample()).unwrap();
        let back = read(&dir).unwrap().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!((back.version, back.seed, back.next_id), (SAVE_VERSION, 42, 7));
        assert_eq!(back.players.len(), 1);
    }

    #[test]
    fn corrupt_file_error_names_the_path() {
        let dir = std::env::temp_dir().join(format!("idk-save-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut bytes = postcard::to_stdvec(&sample()).unwrap();
        bytes.truncate(bytes.len() / 2);
        std::fs::write(dir.join(FILE), bytes).unwrap();
        let err = read(&dir).err().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(err.to_string().contains("world.bin"), "{err}");
    }

    #[test]
    fn unknown_version_is_reported_before_decoding() {
        let mut bytes = postcard::to_stdvec(&999u32).unwrap();
        bytes.extend_from_slice(&[0xff, 0xfe, 0xfd]); // structure différente, non décodable
        let err = decode(&bytes).err().unwrap();
        assert_eq!(err.to_string(), "unsupported save version 999");
    }

    #[test]
    fn version_is_read_without_decoding_the_rest() {
        let mut bytes = postcard::to_stdvec(&SAVE_VERSION).unwrap();
        bytes.extend_from_slice(&[0xff, 0xff, 0xff]);
        // Version reconnue : l'erreur vient du décodage du reste, pas de la version.
        let err = decode(&bytes).err().unwrap();
        assert!(!err.to_string().contains("unsupported save version"));
    }

    #[test]
    fn truncated_chunk_is_rejected() {
        let mut data = sample();
        data.chunks = vec![(ChunkPos::new(3, -2), vec![Tile::Grass; 10])];
        let bytes = postcard::to_stdvec(&data).unwrap();
        let err = decode(&bytes).err().unwrap();
        assert_eq!(err.to_string(), "invalid chunk at (3, -2)");
    }

    #[test]
    fn unknown_tile_id_is_rejected() {
        let mut data = sample();
        data.chunks = vec![(ChunkPos::new(0, 0), vec![Tile::Grass; CHUNK_AREA])];
        let mut bytes = postcard::to_stdvec(&data).unwrap();
        // Dernière tuile de la liste : Grass = 0 ; on la remplace par un id hors table.
        // Après les tuiles viennent `spawned` (len 0) et `animals` (len 0) : 2 octets.
        let i = bytes.len() - 3;
        // < 128 : un seul octet en varint, l'erreur vient bien de l'enum et pas d'une troncature.
        let ok = bytes.clone();
        bytes[i] = 100;
        assert!(decode(&ok).is_ok(), "témoin : la save non modifiée se charge");
        assert!(decode(&bytes).is_err());
    }
}
