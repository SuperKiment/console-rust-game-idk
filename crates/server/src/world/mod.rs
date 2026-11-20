//! Tile storage: chunks generated on demand, modifications tracked for saving.

mod generator;
mod noise;

use std::collections::{HashMap, HashSet};

use idk_shared::content::{Biome, Species, Tile};
use idk_shared::geo::{ChunkPos, Pos};
use idk_shared::protocol::ChunkData;

pub use generator::Generator;

pub struct Chunk {
    pub tiles: Vec<Tile>,
    pub biomes: Vec<Biome>,
    /// Differs from what the generator produces: must be saved.
    pub modified: bool,
    /// Animals generated with the chunk, not yet handed to the game.
    pending_spawns: Vec<(Species, Pos)>,
}

pub struct World {
    pub seed: u64,
    generator: Generator,
    chunks: HashMap<ChunkPos, Chunk>,
    /// Chunks whose animals were already created (survives restarts, prevents duplicates).
    spawned: HashSet<ChunkPos>,
    spawn: std::cell::OnceCell<Pos>,
}

impl World {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            generator: Generator::new(seed),
            chunks: HashMap::new(),
            spawned: HashSet::new(),
            spawn: Default::default(),
        }
    }

    /// Makes sure a chunk exists and hands over its animals the first time it is called for it.
    pub fn load(&mut self, cp: ChunkPos) -> Vec<(Species, Pos)> {
        self.chunk(cp);
        let fresh = self.spawned.insert(cp);
        let chunk = self.chunks.get_mut(&cp).expect("generated above");
        if fresh { std::mem::take(&mut chunk.pending_spawns) } else { Vec::new() }
    }

    /// The chunk, generated if needed (its animals wait in `pending_spawns` until [`World::load`]).
    fn chunk(&mut self, cp: ChunkPos) -> &mut Chunk {
        let (generator, spawned) = (&self.generator, &self.spawned);
        self.chunks.entry(cp).or_insert_with(|| {
            let g = generator.generate(cp);
            // A chunk whose animals were created before a restart regenerates without them.
            let pending_spawns = if spawned.contains(&cp) { Vec::new() } else { g.spawns };
            Chunk { tiles: g.tiles, biomes: g.biomes, modified: false, pending_spawns }
        })
    }

    /// Tile at `p`. Unloaded chunks are generated (their animals are created later, when a player comes near).
    pub fn tile(&mut self, p: Pos) -> Tile {
        self.chunk(p.chunk()).tiles[p.local_index()]
    }

    pub fn biome(&mut self, p: Pos) -> Biome {
        self.chunk(p.chunk()).biomes[p.local_index()]
    }

    pub fn set_tile(&mut self, p: Pos, tile: Tile) {
        let c = self.chunk(p.chunk());
        c.tiles[p.local_index()] = tile;
        c.modified = true;
    }

    pub fn chunk_data(&mut self, cp: ChunkPos) -> ChunkData {
        let c = self.chunk(cp);
        ChunkData { pos: cp, tiles: c.tiles.clone(), biomes: c.biomes.clone() }
    }

    pub fn spawn_point(&self) -> Pos {
        *self.spawn.get_or_init(|| self.generator.spawn_point())
    }

    // --- persistence ---

    pub fn modified_chunks(&self) -> impl Iterator<Item = (ChunkPos, &Vec<Tile>)> {
        self.chunks.iter().filter(|(_, c)| c.modified).map(|(p, c)| (*p, &c.tiles))
    }

    pub fn spawned(&self) -> impl Iterator<Item = ChunkPos> + '_ {
        self.spawned.iter().copied()
    }

    /// Rebuilds a world from saved data: modified chunks keep their tiles, others regenerate.
    pub fn restore(
        seed: u64,
        modified: impl IntoIterator<Item = (ChunkPos, Vec<Tile>)>,
        spawned: impl IntoIterator<Item = ChunkPos>,
    ) -> Self {
        let mut w = World::new(seed);
        w.spawned.extend(spawned);
        for (cp, tiles) in modified {
            let biomes = w.generator.generate(cp).biomes;
            w.chunks.insert(cp, Chunk { tiles, biomes, modified: true, pending_spawns: Vec::new() });
        }
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_a_tile_does_not_lose_the_chunk_animals() {
        let mut w = World::new(9);
        // Find a chunk with animals, touch it through a tile read first, then load it.
        let cp = (0..200)
            .map(|i| ChunkPos::new(i, 0))
            .find(|cp| !World::new(9).load(*cp).is_empty())
            .expect("some chunk has animals");
        let expected = World::new(9).load(cp);
        w.tile(cp.origin());
        assert_eq!(w.load(cp), expected);
        assert!(w.load(cp).is_empty(), "animals are handed over once");
    }

    #[test]
    fn restored_world_does_not_respawn_animals() {
        let cp = ChunkPos::new(0, 0);
        let mut w = World::restore(9, [], [cp]);
        assert!(w.load(cp).is_empty());
    }
}
