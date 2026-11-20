//! What the client knows about the world, rebuilt from server messages.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant, SystemTime};

use idk_shared::content::{Biome, Effect, Tile};
use idk_shared::geo::{ChunkPos, Pos};
use idk_shared::protocol::{ChunkData, EntityId, EntityView, LogLine, ServerMsg};

/// Must match the server's streaming radius + 1 (chunks farther away stop receiving updates).
const KEEP_CHUNKS: i32 = 4;
const LOG_SIZE: usize = 200;

pub struct LogEntry {
    /// Seconds since the Unix epoch.
    pub time: u64,
    pub line: LogLine,
}

pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Default)]
pub struct GameState {
    pub me: Option<EntityId>,
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
            ServerMsg::Rejected(_) => {}
        }
        self.effects.retain(|(_, _, until)| *until > now);
    }

    pub fn push_log(&mut self, line: LogLine) {
        self.log.push_back(LogEntry { time: unix_now(), line });
        if self.log.len() > LOG_SIZE {
            self.log.pop_front();
        }
    }

    pub fn me(&self) -> Option<&EntityView> {
        self.entities.iter().find(|e| Some(e.id) == self.me)
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
