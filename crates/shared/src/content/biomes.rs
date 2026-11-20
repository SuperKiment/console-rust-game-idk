use super::{Species, Tile, content_table};

#[derive(Debug, Clone, Copy)]
pub struct BiomeDef {
    pub name: &'static str,
    /// Elevation band `[min, max)` where the biome can appear (noise roughly in -1..1).
    pub elevation: (f32, f32),
    /// Preferred `(temperature, moisture)` in 0..1. Inside an elevation band the biome whose
    /// climate is closest wins.
    pub climate: (f32, f32),
    /// Weighted tile palette.
    pub tiles: &'static [(Tile, u16)],
    /// Average number of animals of each species spawned per chunk.
    pub spawns: &'static [(Species, f32)],
}

const LAND: (f32, f32) = (-0.18, 0.42);

content_table! {
    /// Biome kinds. Discriminants are the wire format: append new rows at the end.
    Biome: BiomeDef => BIOMES {
        DeepLake => BiomeDef { name: "deep lake", elevation: (-9.0, -0.34), climate: (0.5, 0.5),
            tiles: &[(Tile::DeepWater, 1)], spawns: &[] },
        Lake => BiomeDef { name: "lake", elevation: (-0.34, -0.24), climate: (0.5, 0.5),
            tiles: &[(Tile::Water, 1)], spawns: &[] },
        Shore => BiomeDef { name: "shore", elevation: (-0.24, LAND.0), climate: (0.5, 0.5),
            tiles: &[(Tile::Sand, 30), (Tile::Gravel, 2)], spawns: &[(Species::Chicken, 0.1)] },
        Plains => BiomeDef { name: "plains", elevation: LAND, climate: (0.55, 0.45),
            tiles: &[(Tile::Grass, 80), (Tile::TallGrass, 14), (Tile::Flowers, 4), (Tile::Tree, 2)],
            spawns: &[(Species::Cow, 1.2), (Species::Sheep, 0.8), (Species::Chicken, 0.6), (Species::Dog, 0.15), (Species::Rabbit, 0.4)] },
        Forest => BiomeDef { name: "forest", elevation: LAND, climate: (0.55, 0.8),
            tiles: &[(Tile::Grass, 52), (Tile::Tree, 26), (Tile::TallGrass, 14), (Tile::Pine, 5), (Tile::Flowers, 3)],
            spawns: &[(Species::Rabbit, 1.0), (Species::Dog, 0.2)] },
        Taiga => BiomeDef { name: "taiga", elevation: LAND, climate: (0.2, 0.7),
            tiles: &[(Tile::Snow, 50), (Tile::Pine, 22), (Tile::Grass, 20), (Tile::Gravel, 3)],
            spawns: &[(Species::Rabbit, 0.6), (Species::Sheep, 0.3)] },
        Tundra => BiomeDef { name: "tundra", elevation: LAND, climate: (0.05, 0.3),
            tiles: &[(Tile::Snow, 85), (Tile::Gravel, 10), (Tile::Rock, 2), (Tile::Pine, 1)],
            spawns: &[(Species::Rabbit, 0.3)] },
        Desert => BiomeDef { name: "desert", elevation: LAND, climate: (0.95, 0.1),
            tiles: &[(Tile::Sand, 90), (Tile::Cactus, 2), (Tile::Gravel, 4), (Tile::Rock, 1)],
            spawns: &[] },
        Meadow => BiomeDef { name: "meadow", elevation: LAND, climate: (0.75, 0.6),
            tiles: &[(Tile::Grass, 55), (Tile::TallGrass, 25), (Tile::Flowers, 18), (Tile::Tree, 1)],
            spawns: &[(Species::Sheep, 1.0), (Species::Cow, 0.6), (Species::Rabbit, 0.6)] },
        Hills => BiomeDef { name: "hills", elevation: (LAND.1, 0.52), climate: (0.5, 0.5),
            tiles: &[(Tile::Gravel, 45), (Tile::Grass, 35), (Tile::Rock, 18), (Tile::Pine, 2)],
            spawns: &[(Species::Sheep, 0.5)] },
        Mountain => BiomeDef { name: "mountain", elevation: (0.52, 0.64), climate: (0.5, 0.5),
            tiles: &[(Tile::Rock, 70), (Tile::Gravel, 30)], spawns: &[] },
        Peak => BiomeDef { name: "peak", elevation: (0.64, 9.0), climate: (0.5, 0.5),
            tiles: &[(Tile::Rock, 55), (Tile::Snow, 45)], spawns: &[] },
    }
}

impl Biome {
    /// Biome for a given elevation and climate, following the table's bands and climates.
    pub fn select(elevation: f32, temperature: f32, moisture: f32) -> Biome {
        let d = |b: &BiomeDef| (b.climate.0 - temperature).powi(2) + (b.climate.1 - moisture).powi(2);
        *Biome::ALL
            .iter()
            .filter(|b| (b.def().elevation.0..b.def().elevation.1).contains(&elevation))
            .min_by(|a, b| d(a.def()).total_cmp(&d(b.def())))
            .expect("biome elevation bands must cover the whole range")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevation_bands_cover_range() {
        for i in -100..=100 {
            Biome::select(i as f32 / 50.0, 0.5, 0.5);
        }
    }

    #[test]
    fn climate_picks_nearest() {
        assert_eq!(Biome::select(0.0, 0.95, 0.05), Biome::Desert);
        assert_eq!(Biome::select(0.0, 0.0, 0.3), Biome::Tundra);
        assert_eq!(Biome::select(-0.5, 0.0, 0.0), Biome::DeepLake);
    }
}
