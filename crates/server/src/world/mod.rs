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

    /// Makes sure a chunk exists. Returns the animals to create if it is generated for the very first time.
    pub fn load(&mut self, cp: ChunkPos) -> Vec<(Species, Pos)> {
        if self.chunks.contains_key(&cp) {
            return Vec::new();
        }
        let g = self.generator.generate(cp);
        self.chunks.insert(cp, Chunk { tiles: g.tiles, biomes: g.biomes, modified: false });
        if self.spawned.insert(cp) { g.spawns } else { Vec::new() }
    }

    fn chunk(&mut self, cp: ChunkPos) -> &mut Chunk {
        self.load(cp);
        self.chunks.get_mut(&cp).expect("chunk loaded above")
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
            w.chunks.insert(cp, Chunk { tiles, biomes, modified: true });
        }
        w
    }
}
