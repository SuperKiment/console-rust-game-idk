//! World persistence: one postcard file, written atomically.

use std::io;
use std::path::Path;
use std::time::Instant;

use idk_shared::content::{Species, Tile};
use idk_shared::geo::{ChunkPos, Dir, Pos};
use idk_shared::protocol::EntityId;
use serde::{Deserialize, Serialize};

use crate::game::{Animal, Game};
use crate::world::World;

/// Bump on incompatible changes and convert older versions in [`read`].
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
            animals: self.animals.values().map(|a| (a.species, a.pos, a.tamed_by.clone())).collect(),
        }
    }

    pub fn from_save(s: SaveData, now: Instant) -> Game {
        let mut g = Game::new(World::restore(s.seed, s.chunks, s.spawned));
        g.next_id = s.next_id;
        g.offline = s.players.into_iter().map(|(n, p, d)| (n, (p, d))).collect();
        for (species, pos, tamed_by) in s.animals {
            let id = g.alloc_id();
            g.animals.insert(id, Animal { id, species, pos, facing: Dir::South, tamed_by, next_step: now });
        }
        g
    }
}

pub fn read(dir: &Path) -> io::Result<Option<SaveData>> {
    let bytes = match std::fs::read(dir.join(FILE)) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        r => r?,
    };
    let data: SaveData = postcard::from_bytes(&bytes).map_err(io::Error::other)?;
    if data.version != SAVE_VERSION {
        return Err(io::Error::other(format!("unsupported save version {}", data.version)));
    }
    Ok(Some(data))
}

pub fn write(dir: &Path, data: &SaveData) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let bytes = postcard::to_stdvec(data).map_err(io::Error::other)?;
    let tmp = dir.join(format!("{FILE}.tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, dir.join(FILE))
}
