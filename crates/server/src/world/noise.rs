//! Seeded 2D value noise with fractal octaves.

use crate::rng::{hash2, unit};

fn lattice(seed: u64, x: i32, y: i32) -> f32 {
    unit(hash2(seed, x, y)) * 2.0 - 1.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Smooth value noise in `[-1, 1]`, period 1 lattice.
pub fn value(seed: u64, x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (smooth(x - x0), smooth(y - y0));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let top = lerp(lattice(seed, ix, iy), lattice(seed, ix + 1, iy), tx);
    let bottom = lerp(lattice(seed, ix, iy + 1), lattice(seed, ix + 1, iy + 1), tx);
    lerp(top, bottom, ty)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Fractal sum of `octaves` layers; `scale` is the size in tiles of the largest features.
/// Result is normalized to roughly `[-1, 1]`.
pub fn fbm(seed: u64, x: i32, y: i32, scale: f32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, 1.0 / scale);
    for o in 0..octaves {
        sum += amp * value(seed.wrapping_add(o as u64 * 7919), x as f32 * freq, y as f32 * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}
