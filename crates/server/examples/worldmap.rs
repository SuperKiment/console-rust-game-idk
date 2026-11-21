//! Prints an overview of a world: `cargo run -p idk-server --example worldmap -- <seed> [step]`.
//! Each character is the biome sampled every `step` tiles (default 16), `@` is the origin; a legend follows.

use std::collections::HashMap;

use idk_server::world::Generator;
use idk_shared::content::Biome;
use idk_shared::geo::Pos;

/// One distinct character per biome. Exhaustive: a new biome breaks the build until it gets one.
fn glyph(b: Biome) -> char {
    match b {
        Biome::DeepLake => '~',
        Biome::Lake => '-',
        Biome::Shore => '.',
        Biome::Plains => '"',
        Biome::Forest => 'T',
        Biome::Taiga => 'A',
        Biome::Tundra => '_',
        Biome::Desert => ':',
        Biome::Meadow => ',',
        Biome::Hills => 'n',
        Biome::Mountain => '^',
        Biome::Peak => 'M',
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let step: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(16);
    let g = Generator::new(seed);
    let mut counts = HashMap::new();
    for y in -30..30 {
        let line: String = (-60..60)
            .map(|x| {
                let b = g.biome(Pos::new(x * step, y * step));
                *counts.entry(b).or_insert(0) += 1;
                if x == 0 && y == 0 { '@' } else { glyph(b) }
            })
            .collect();
        println!("{line}");
    }
    println!("@ origin");
    let mut legend: Vec<_> = counts.into_iter().collect();
    legend.sort_by_key(|(b, _)| *b as u8);
    for (b, n) in legend {
        println!("{} {} ({n})", glyph(b), b.def().name);
    }
}
