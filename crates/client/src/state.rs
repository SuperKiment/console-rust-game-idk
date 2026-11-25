//! What the client knows about the world, rebuilt from server messages.

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};

use idk_shared::content::{Biome, Effect, Tile};
use idk_shared::geo::{ChunkPos, Dir, Pos};
use idk_shared::protocol::{ChunkData, EntityId, EntityView, KEEP_CHUNKS, LogKind, LogLine, ServerMsg};

const LOG_SIZE: usize = 200;

pub struct LogEntry {
    /// Seconds since the Unix epoch.
    pub time: u64,
    pub line: LogLine,
    /// Couleur (index palette) de l'auteur d'un chat, figée à la réception.
    pub color: u8,
}

pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Décalage local − UTC en secondes, lu une seule fois (un changement d'heure en cours de partie
/// n'est pas suivi). Fuseau de la machine : `TZ` ou `/etc/localtime` (monté par docker compose).
pub fn utc_offset() -> i64 {
    static OFFSET: OnceLock<i64> = OnceLock::new();
    *OFFSET.get_or_init(|| chrono::Local::now().offset().local_minus_utc() as i64)
}

/// Secondes Unix décalées de `offset` : à passer à `ui::clock` pour l'heure locale.
pub fn shifted(unix: u64, offset: i64) -> u64 {
    unix.saturating_add_signed(offset)
}

#[derive(Default)]
pub struct GameState {
    pub me: Option<EntityId>,
    /// Dernière position connue, gardée à la déconnexion pour dessiner la carte grisée.
    last_pos: Option<Pos>,
    /// Direction posée localement au dernier Move ; `None` tant qu'aucun Move depuis le Welcome.
    local_facing: Option<Dir>,
    pub name: String,
    pub seed: u64,
    chunks: HashMap<ChunkPos, ChunkData>,
    pub entities: Vec<EntityView>,
    pub online: Vec<(String, u8)>,
    pub log: VecDeque<LogEntry>,
    effects: Vec<(Pos, Effect, Instant)>,
}

impl GameState {
    pub fn apply(&mut self, msg: ServerMsg, now: Instant) {
        match msg {
            ServerMsg::Welcome { id, name, seed } => {
                *self = GameState {
                    me: Some(id),
                    name,
                    seed,
                    log: std::mem::take(&mut self.log),
                    ..Default::default()
                };
            }
            ServerMsg::Chunk(c) if c.is_valid() => {
                self.chunks.insert(c.pos, c);
                if let Some(me) = self.my_pos() {
                    self.chunks.retain(|cp, _| cp.dist(me.chunk()) <= KEEP_CHUNKS);
                }
            }
            ServerMsg::Chunk(_) => {}
            ServerMsg::TileChanged { pos, tile } => {
                if let Some(c) = self.chunks.get_mut(&pos.chunk()) {
                    c.tiles[pos.local_index()] = tile;
                }
            }
            ServerMsg::Entities(es) => self.entities = es,
            ServerMsg::Log(line) => self.push_log(line),
            ServerMsg::Effect { pos, effect } => {
                let until = now + Duration::from_millis(effect.def().duration_ms as u64);
                self.effects.push((pos, effect, until));
            }
            ServerMsg::Online(list) => self.online = list,
            // Inatteignable : `net::session` intercepte `Rejected` avant l'état.
            ServerMsg::Rejected(_) => {}
        }
        self.effects.retain(|(_, _, until)| *until > now);
    }

    /// Connexion perdue : on garde chunks et position pour une carte grisée, sans entités ni effets.
    /// Appelé à chaque échec de reconnexion : `me` est alors déjà `None`, on garde la dernière position.
    pub fn disconnect(&mut self) {
        if let Some(p) = self.my_pos() {
            self.last_pos = Some(p);
        }
        self.me = None;
        self.entities.clear();
        self.online.clear();
        self.effects.clear();
    }

    /// Hors ligne = plus d'identité mais une position gardée (`None` avant le premier Welcome).
    pub fn is_offline(&self) -> bool {
        self.me.is_none() && self.last_pos.is_some()
    }

    /// Centre de la carte : ma position, sinon la dernière connue.
    pub fn map_center(&self) -> Option<Pos> {
        self.my_pos().or(self.last_pos)
    }

    pub fn push_log(&mut self, line: LogLine) {
        let color = match &line.kind {
            LogKind::Chat { from } => self.online.iter().find(|(n, _)| n == from).map_or(0, |(_, c)| *c),
            _ => 0,
        };
        self.log.push_back(LogEntry { time: unix_now(), line, color });
        if self.log.len() > LOG_SIZE {
            self.log.pop_front();
        }
    }

    pub fn me(&self) -> Option<&EntityView> {
        self.entities.iter().find(|e| Some(e.id) == self.me)
    }

    /// Direction regardée : le dernier Move local prime sur le snapshot serveur (en retard).
    /// Le serveur fixe `facing` à chaque Move, même bloqué ou en cooldown, donc pas de divergence.
    pub fn facing(&self) -> Dir {
        self.local_facing.or(self.me().map(|e| e.facing)).unwrap_or_default()
    }

    pub fn set_facing(&mut self, dir: Dir) {
        self.local_facing = Some(dir);
    }

    pub fn my_pos(&self) -> Option<Pos> {
        self.me().map(|e| e.pos)
    }

    pub fn tile(&self, p: Pos) -> Option<Tile> {
        self.chunks.get(&p.chunk()).map(|c| c.tiles[p.local_index()])
    }

    pub fn biome(&self, p: Pos) -> Option<Biome> {
        self.chunks.get(&p.chunk()).map(|c| c.biomes[p.local_index()])
    }

    pub fn entity_at(&self, p: Pos) -> Option<&EntityView> {
        // Players are drawn above animals.
        let mut at = self.entities.iter().filter(|e| e.pos == p);
        let first = at.next()?;
        Some(at.find(|e| matches!(e.look, idk_shared::protocol::Look::Player { .. })).unwrap_or(first))
    }

    pub fn effect_at(&self, p: Pos, now: Instant) -> Option<Effect> {
        self.effects.iter().rev().find(|(ep, _, until)| *ep == p && *until > now).map(|(_, e, _)| *e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_color_is_kept_after_author_leaves() {
        let mut s = GameState::default();
        let now = Instant::now();
        s.apply(ServerMsg::Online(vec![("bob".into(), 3)]), now);
        s.apply(ServerMsg::Log(LogLine::new(LogKind::Chat { from: "bob".into() }, "hi")), now);
        s.apply(ServerMsg::Online(vec![]), now);
        assert_eq!(s.log.back().unwrap().color, 3);
    }

    #[test]
    fn repeated_disconnect_keeps_map_center() {
        let mut s = GameState::default();
        let now = Instant::now();
        let pos = Pos::new(3, 4);
        s.apply(ServerMsg::Welcome { id: 1, name: "a".into(), seed: 0 }, now);
        s.apply(
            ServerMsg::Entities(vec![EntityView {
                id: 1,
                pos,
                facing: Dir::North,
                look: idk_shared::protocol::Look::Player { name: "a".into(), color: 0 },
            }]),
            now,
        );
        s.disconnect();
        s.disconnect();
        assert!(s.is_offline());
        assert_eq!(s.map_center(), Some(pos));
    }

    use idk_shared::geo::CHUNK_AREA;
    use idk_shared::protocol::Look;

    fn chunk(x: i32, y: i32) -> ChunkData {
        ChunkData {
            pos: ChunkPos::new(x, y),
            tiles: vec![Tile::Grass; CHUNK_AREA],
            biomes: vec![Biome::Plains; CHUNK_AREA],
        }
    }

    /// État avec un joueur (id 1) placé en `pos`.
    fn state_at(pos: Pos, now: Instant) -> GameState {
        let mut s = GameState::default();
        s.apply(ServerMsg::Welcome { id: 1, name: "a".into(), seed: 0 }, now);
        let look = Look::Player { name: "a".into(), color: 0 };
        s.apply(ServerMsg::Entities(vec![EntityView { id: 1, pos, facing: Dir::North, look }]), now);
        s
    }

    #[test]
    fn invalid_chunk_is_rejected() {
        let now = Instant::now();
        let mut s = GameState::default();
        let mut bad = chunk(0, 0);
        bad.tiles.pop();
        s.apply(ServerMsg::Chunk(bad), now);
        assert_eq!(s.tile(Pos::new(0, 0)), None);
        s.apply(ServerMsg::Chunk(chunk(0, 0)), now);
        assert_eq!(s.tile(Pos::new(0, 0)), Some(Tile::Grass));
    }

    #[test]
    fn far_chunks_are_evicted() {
        let now = Instant::now();
        let mut s = state_at(Pos::new(0, 0), now);
        s.apply(ServerMsg::Chunk(chunk(KEEP_CHUNKS, 0)), now);
        s.apply(ServerMsg::Chunk(chunk(KEEP_CHUNKS + 1, 0)), now);
        let tile_x = |c: i32| Pos::new(c * idk_shared::geo::CHUNK_SIZE, 0);
        assert!(s.tile(tile_x(KEEP_CHUNKS)).is_some(), "à KEEP_CHUNKS : gardé");
        assert!(s.tile(tile_x(KEEP_CHUNKS + 1)).is_none(), "au-delà de KEEP_CHUNKS : évincé");
    }

    #[test]
    fn tile_changed_updates_loaded_chunk_and_ignores_unloaded() {
        let now = Instant::now();
        let mut s = GameState::default();
        s.apply(ServerMsg::Chunk(chunk(0, 0)), now);
        s.apply(ServerMsg::TileChanged { pos: Pos::new(1, 1), tile: Tile::Wall }, now);
        assert_eq!(s.tile(Pos::new(1, 1)), Some(Tile::Wall));
        s.apply(ServerMsg::TileChanged { pos: Pos::new(500, 500), tile: Tile::Wall }, now);
        assert_eq!(s.tile(Pos::new(500, 500)), None, "chunk non chargé : pas créé");
    }

    #[test]
    fn effect_expires_after_its_duration() {
        let now = Instant::now();
        let mut s = GameState::default();
        let pos = Pos::new(2, 2);
        s.apply(ServerMsg::Effect { pos, effect: Effect::Heart }, now);
        let dur = Duration::from_millis(Effect::Heart.def().duration_ms as u64);
        assert_eq!(s.effect_at(pos, now), Some(Effect::Heart));
        assert_eq!(s.effect_at(pos, now + dur - Duration::from_millis(1)), Some(Effect::Heart));
        assert_eq!(s.effect_at(pos, now + dur), None);
        // Le prochain message purge l'effet expiré.
        s.apply(ServerMsg::Online(vec![]), now + dur);
        assert!(s.effects.is_empty());
    }

    #[test]
    fn log_is_capped_and_drops_oldest() {
        let now = Instant::now();
        let mut s = GameState::default();
        for i in 0..LOG_SIZE + 5 {
            s.apply(ServerMsg::Log(LogLine::new(LogKind::Chat { from: "x".into() }, format!("{i}"))), now);
        }
        assert_eq!(s.log.len(), LOG_SIZE);
        assert_eq!(s.log.front().unwrap().line.text, "5");
        assert_eq!(s.log.back().unwrap().line.text, (LOG_SIZE + 4).to_string());
    }
}
