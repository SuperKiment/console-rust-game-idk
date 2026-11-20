//! Procedural chunk generation, driven entirely by the biome table.

use idk_shared::content::{Biome, Species, Tile};
use idk_shared::geo::{CHUNK_AREA, CHUNK_SIZE, ChunkPos, Pos};

use super::noise::fbm;
use crate::rng::{Rng, hash2};

pub struct Generator {
    seed: u64,
}

pub struct Generated {
    pub tiles: Vec<Tile>,
    pub biomes: Vec<Biome>,
    /// Animals to create the first time this chunk exists.
    pub spawns: Vec<(Species, Pos)>,
}

impl Generator {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub fn biome(&self, p: Pos) -> Biome {
        let s = self.seed;
        let elevation = fbm(s, p.x, p.y, 160.0, 5);
        let temperature = fbm(s ^ 0x7E3A, p.x, p.y, 420.0, 3) * 1.2 + 0.5;
        let moisture = fbm(s ^ 0x5C11, p.x, p.y, 300.0, 3) * 1.2 + 0.5;
        Biome::select(elevation, temperature, moisture)
    }

    pub fn tile(&self, p: Pos, biome: Biome) -> Tile {
        let palette = biome.def().tiles;
        let total: u32 = palette.iter().map(|(_, w)| *w as u32).sum();
        let mut roll = (hash2(self.seed ^ 0x711E, p.x, p.y) % total as u64) as u32;
        for (tile, w) in palette {
            if roll < *w as u32 {
                return *tile;
            }
            roll -= *w as u32;
        }
        unreachable!("weights sum to total")
    }

    pub fn generate(&self, cp: ChunkPos) -> Generated {
        let biomes: Vec<Biome> = (0..CHUNK_AREA).map(|i| self.biome(cp.tile_pos(i))).collect();
        let tiles: Vec<Tile> =
            biomes.iter().enumerate().map(|(i, b)| self.tile(cp.tile_pos(i), *b)).collect();

        let mut rng = Rng::new(hash2(self.seed ^ 0x5BA3, cp.x, cp.y));
        let mut spawns = Vec::new();
        let center = biomes[CHUNK_AREA / 2 + CHUNK_SIZE as usize / 2];
        for (species, avg) in center.def().spawns {
            let count = avg.floor() as usize + rng.chance(avg.fract()) as usize;
            for _ in 0..count {
                // A few attempts to land on walkable ground of the right biome.
                let spot = (0..8).map(|_| rng.below(CHUNK_AREA)).find(|&i| {
                    tiles[i].def().walkable
                        && biomes[i] == center
                        && !spawns.iter().any(|(_, p)| *p == cp.tile_pos(i))
                });
                if let Some(i) = spot {
                    spawns.push((*species, cp.tile_pos(i)));
                }
            }
        }
        Generated { tiles, biomes, spawns }
    }

    /// Walkable tile near the origin where new players appear, preferably in a welcoming biome.
    pub fn spawn_point(&self) -> Pos {
        let ok = |p: Pos, biomes: &[Biome]| {
            let b = self.biome(p);
            biomes.contains(&b) && self.tile(p, b).def().walkable
        };
        let land: Vec<Biome> =
            Biome::ALL.iter().copied().filter(|b| b.def().elevation.0 >= LAND_MIN).collect();
        for biomes in [SPAWN_BIOMES, &land] {
            // Expanding square rings around (0, 0), sampled every 8 tiles.
            for r in (0..4000i32).step_by(8) {
                for i in (-r..=r).step_by(8) {
                    for p in [Pos::new(i, -r), Pos::new(i, r), Pos::new(-r, i), Pos::new(r, i)] {
                        if ok(p, biomes) {
                            return p;
                        }
                    }
                }
            }
        }
        Pos::default()
    }
}

/// Biomes new players preferably appear in.
const SPAWN_BIOMES: &[Biome] = &[Biome::Plains, Biome::Meadow];
/// Lowest elevation considered dry land for the fallback spawn search.
const LAND_MIN: f32 = -0.3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic() {
        let a = Generator::new(42).generate(ChunkPos::new(3, -2));
        let b = Generator::new(42).generate(ChunkPos::new(3, -2));
        assert_eq!(a.tiles, b.tiles);
        assert_eq!(a.spawns, b.spawns);
        assert_ne!(Generator::new(43).generate(ChunkPos::new(3, -2)).tiles, a.tiles);
    }

    #[test]
    fn spawn_point_is_walkable() {
        for seed in 0..5 {
            let g = Generator::new(seed);
            let p = g.spawn_point();
            assert!(g.tile(p, g.biome(p)).def().walkable);
        }
    }

    #[test]
    fn every_biome_appears_somewhere() {
        // Coarse survey across a few seeds: catches thresholds that make a biome unreachable.
        let mut seen = std::collections::HashSet::new();
        for seed in 0..3 {
            let g = Generator::new(seed);
            for y in (-4000..4000).step_by(40) {
                for x in (-4000..4000).step_by(40) {
                    seen.insert(g.biome(Pos::new(x, y)));
                }
            }
        }
        let missing: Vec<_> = Biome::ALL.iter().filter(|b| !seen.contains(b)).map(|b| b.def().name).collect();
        assert!(missing.is_empty(), "unreachable biomes: {missing:?}");
    }
}
