//! Small deterministic PRNG and hash helpers (no external dependency needed).

/// SplitMix64 finalizer: good avalanche, used for coordinate hashing.
pub fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Hash of a seed and a 2D integer coordinate.
pub fn hash2(seed: u64, x: i32, y: i32) -> u64 {
    mix(seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
}

/// Uniform float in `[0, 1)` from a hash.
pub fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    pub fn chance(&mut self, p: f32) -> bool {
        unit(self.next_u64()) < p
    }

    /// Uniform integer in `0..n` (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        (!items.is_empty()).then(|| &items[self.below(items.len())])
    }
}
