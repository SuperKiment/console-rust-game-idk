//! Messages exchanged between client and server.
//!
//! Each message is serialized with postcard and framed with a 4-byte length prefix
//! (see [`crate::net`]). Bump [`PROTOCOL_VERSION`] on any incompatible change.

use serde::{Deserialize, Serialize};

use crate::content::{Biome, Effect, Species, Tile};
use crate::geo::{CHUNK_AREA, ChunkPos, Dir, Pos};

pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_PORT: u16 = 7777;
/// Radius (in chunks) of the area the server streams around each player.
pub const VIEW_CHUNKS: i32 = 3;
/// Chunks farther than this are forgotten by both sides: the server resends them on return.
/// The client must not drop a chunk the server still considers sent, hence a single constant.
pub const KEEP_CHUNKS: i32 = VIEW_CHUNKS + 1;

/// Identifier of anything that moves (players and animals share the id space).
pub type EntityId = u32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// First message on a connection.
    Hello {
        version: u32,
        name: String,
    },
    /// Step one tile (or just turn, if blocked).
    Move(Dir),
    /// Pet / talk to whatever stands on the adjacent tile.
    Interact(Dir),
    /// Place a wall on the adjacent tile.
    Build(Dir),
    /// Break the adjacent tile.
    Break(Dir),
    Chat(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome {
        id: EntityId,
        name: String,
        seed: u64,
    },
    /// Full content of a chunk the player is near.
    Chunk(ChunkData),
    TileChanged {
        pos: Pos,
        tile: Tile,
    },
    /// Every entity close to the player, sent each tick.
    Entities(Vec<EntityView>),
    Log(LogLine),
    Effect {
        pos: Pos,
        effect: Effect,
    },
    /// Everyone connected, as `(name, color)`. Sent on every join and leave.
    Online(Vec<(String, u8)>),
    /// Connection refused (e.g. protocol mismatch). The server closes the socket after it.
    Rejected(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkData {
    pub pos: ChunkPos,
    /// `CHUNK_AREA` tiles, row-major.
    pub tiles: Vec<Tile>,
    /// `CHUNK_AREA` biomes, row-major.
    pub biomes: Vec<Biome>,
}

impl ChunkData {
    pub fn is_valid(&self) -> bool {
        self.tiles.len() == CHUNK_AREA && self.biomes.len() == CHUNK_AREA
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityView {
    pub id: EntityId,
    pub pos: Pos,
    pub facing: Dir,
    pub look: Look,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Look {
    Player { name: String, color: u8 },
    Animal { species: Species, tamed_by: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    pub kind: LogKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogKind {
    /// Server notices (joins, leaves); the client also uses it for its own connection notices.
    System,
    /// Result of the player's own actions.
    Info,
    Chat {
        from: String,
    },
    /// Animal sounds and reactions.
    Animal,
}

impl LogLine {
    pub fn new(kind: LogKind, text: impl Into<String>) -> Self {
        Self { kind, text: text.into() }
    }
}
