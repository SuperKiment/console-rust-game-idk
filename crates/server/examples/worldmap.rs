//! Prints an overview of a world: `cargo run -p idk-server --example worldmap -- <seed> [step]`.
//! Each character is the first letter of the biome sampled every `step` tiles (default 16).

use std::collections::BTreeMap;

use idk_server::world::Generator;
use idk_shared::geo::Pos;

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let step: i32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(16);
    let g = Generator::new(seed);
    let mut counts = BTreeMap::new();
    for y in -30..30 {
        let line: String = (-60..60)
            .map(|x| {
                let b = g.biome(Pos::new(x * step, y * step));
                *counts.entry(b.def().name).or_insert(0) += 1;
                if x == 0 && y == 0 { '@' } else { b.def().name.chars().next().unwrap() }
            })
            .collect();
        println!("{line}");
    }
    println!("{counts:?}");
}
