//! World coordinates, chunks and directions.

use serde::{Deserialize, Serialize};

/// Side length of a square chunk, in tiles.
pub const CHUNK_SIZE: i32 = 32;
/// Number of tiles in a chunk.
pub const CHUNK_AREA: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

/// Absolute tile position in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn step(self, dir: Dir) -> Self {
        let (dx, dy) = dir.delta();
        Self::new(self.x + dx, self.y + dy)
    }

    pub fn chunk(self) -> ChunkPos {
        ChunkPos { x: self.x.div_euclid(CHUNK_SIZE), y: self.y.div_euclid(CHUNK_SIZE) }
    }

    /// Index of this tile inside its chunk's tile array.
    pub fn local_index(self) -> usize {
        (self.y.rem_euclid(CHUNK_SIZE) * CHUNK_SIZE + self.x.rem_euclid(CHUNK_SIZE)) as usize
    }

    /// Chebyshev distance (king moves).
    pub fn dist(self, other: Pos) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }

    /// Cardinal direction that best moves `self` toward `target`, if not already there.
    pub fn dir_to(self, target: Pos) -> Option<Dir> {
        let (dx, dy) = (target.x - self.x, target.y - self.y);
        match (dx, dy) {
            (0, 0) => None,
            _ if dx.abs() >= dy.abs() => Some(if dx > 0 { Dir::East } else { Dir::West }),
            _ => Some(if dy > 0 { Dir::South } else { Dir::North }),
        }
    }
}

/// Chunk coordinates (a chunk covers `CHUNK_SIZE`² tiles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ChunkPos {
    pub x: i32,
    pub y: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// World position of the chunk's top-left tile.
    pub fn origin(self) -> Pos {
        Pos::new(self.x * CHUNK_SIZE, self.y * CHUNK_SIZE)
    }

    /// World position of the tile at `index` in the chunk's tile array.
    pub fn tile_pos(self, index: usize) -> Pos {
        let o = self.origin();
        let i = index as i32;
        Pos::new(o.x + i % CHUNK_SIZE, o.y + i / CHUNK_SIZE)
    }

    /// All chunks within `radius` (Chebyshev) of `self`, including itself.
    pub fn around(self, radius: i32) -> impl Iterator<Item = ChunkPos> {
        (-radius..=radius)
            .flat_map(move |dy| (-radius..=radius).map(move |dx| ChunkPos::new(self.x + dx, self.y + dy)))
    }

    pub fn dist(self, other: ChunkPos) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Dir {
    North,
    #[default]
    South,
    East,
    West,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::North, Dir::South, Dir::East, Dir::West];

    pub const fn delta(self) -> (i32, i32) {
        match self {
            Dir::North => (0, -1),
            Dir::South => (0, 1),
            Dir::East => (1, 0),
            Dir::West => (-1, 0),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Dir::North => "north",
            Dir::South => "south",
            Dir::East => "east",
            Dir::West => "west",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_coordinates_map_to_correct_chunk() {
        let p = Pos::new(-1, -33);
        assert_eq!(p.chunk(), ChunkPos::new(-1, -2));
        assert_eq!(p.chunk().tile_pos(p.local_index()), p);
    }

    #[test]
    fn local_index_roundtrip() {
        for (x, y) in [(0, 0), (31, 31), (32, 5), (-7, 70), (-64, -1)] {
            let p = Pos::new(x, y);
            assert_eq!(p.chunk().tile_pos(p.local_index()), p);
        }
    }

    #[test]
    fn dir_to_prefers_larger_axis() {
        let o = Pos::new(0, 0);
        assert_eq!(o.dir_to(Pos::new(5, 2)), Some(Dir::East));
        assert_eq!(o.dir_to(Pos::new(1, -4)), Some(Dir::North));
        assert_eq!(o.dir_to(o), None);
    }
}
