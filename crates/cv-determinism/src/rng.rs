//! Owned, forkable, label-addressed pseudo-random generator — the **one legal source of randomness**.
//!
//! Design (Design/v0.1): each stream carries an immutable `key` (its identity) and a private `state`
//! that advances as it is drawn. `fork(label)` derives a child stream's key from `(parent key,
//! hash(label))` — depending only on the parent's *identity* and the label, never on how much the
//! parent has been consumed or on the order siblings are forked. That makes **label-addressed
//! sub-streams refactor-safe**: `rng.fork("enemies")` yields the same stream no matter where or when it
//! is called. All arithmetic is fixed-width integer (plus a power-of-two float scale for `[0, 1)`), so
//! output is bit-identical on native and WASM.

/// SplitMix64 finalizer — a strong 64-bit avalanche mix.
#[inline]
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a 64-bit over raw bytes — deterministic, platform-independent label hashing.
#[inline]
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// The SplitMix64 increment (odd, ~golden ratio).
const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
/// Domain separator so `fork_index(0)` never collides with `fork("")`.
const INDEX_SALT: u64 = 0xD1B5_4A32_D192_ED03;
/// 2^-53, exact — scales a 53-bit integer into `[0, 1)`.
const F64_SCALE: f64 = 1.0 / 9_007_199_254_740_992.0;

/// A deterministic pseudo-random stream. Cheap to `fork` and `clone`.
#[derive(Clone, Debug)]
pub struct Rng {
    /// Immutable stream identity — the basis for forking.
    key: u64,
    /// SplitMix64 running state — advances as the stream is drawn.
    state: u64,
}

impl Rng {
    /// The root stream for a seed.
    pub fn new(seed: u64) -> Self {
        let key = mix64(seed);
        Rng { key, state: key }
    }

    /// The stream's immutable identity (handy for tracing / debugging).
    #[inline]
    pub fn key(&self) -> u64 {
        self.key
    }

    /// Draw the next 64 bits (SplitMix64).
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        mix64(self.state)
    }

    /// Draw the next 32 bits (the better-distributed high half).
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Fork a labelled child stream. Order-independent and consumption-independent.
    pub fn fork(&self, label: &str) -> Self {
        let child_key = mix64(self.key ^ mix64(fnv1a(label.as_bytes())));
        Rng {
            key: child_key,
            state: child_key,
        }
    }

    /// Fork a numerically-indexed child stream (for per-element sub-streams without a string label).
    pub fn fork_index(&self, index: u64) -> Self {
        let child_key = mix64(self.key ^ mix64(index ^ INDEX_SALT));
        Rng {
            key: child_key,
            state: child_key,
        }
    }

    /// A double in `[0, 1)` from the high 53 bits (exact power-of-two scaling; no FMA).
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * F64_SCALE
    }

    /// Uniform double in `[lo, hi)`.
    pub fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// Symmetric jitter in `[-radius, radius)`.
    pub fn jitter(&mut self, radius: f64) -> f64 {
        (self.next_f64() * 2.0 - 1.0) * radius
    }

    /// `true` with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// Unbiased integer in `[0, bound)` via Lemire's multiply-shift with rejection. `bound` must be > 0.
    pub fn below(&mut self, bound: u64) -> u64 {
        assert!(bound > 0, "below(0) is undefined");
        // (2^64 - bound) % bound — the rejection threshold that removes modulo bias.
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let x = self.next_u64();
            let m = (x as u128) * (bound as u128);
            let low = m as u64;
            if low >= threshold {
                return (m >> 64) as u64;
            }
        }
    }

    /// Unbiased integer in `[lo, hi)`. Requires `lo < hi`.
    pub fn range_u64(&mut self, lo: u64, hi: u64) -> u64 {
        assert!(lo < hi, "range_u64 requires lo < hi");
        lo + self.below(hi - lo)
    }

    /// Unbiased integer in `[lo, hi)` for signed values. Requires `lo < hi`.
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        assert!(lo < hi, "range_i64 requires lo < hi");
        let span = (hi as i128 - lo as i128) as u64;
        (lo as i128 + self.below(span) as i128) as i64
    }

    /// A weighted index in `[0, weights.len())`. Weights must be non-negative with a positive sum.
    pub fn weighted_choice(&mut self, weights: &[f64]) -> usize {
        let total: f64 = weights.iter().sum();
        assert!(total > 0.0, "weighted_choice needs a positive weight sum");
        let mut r = self.next_f64() * total;
        for (i, &w) in weights.iter().enumerate() {
            r -= w;
            if r < 0.0 {
                return i;
            }
        }
        weights.len() - 1 // floating-point tail guard
    }

    /// In-place Fisher–Yates shuffle.
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        let n = slice.len();
        if n < 2 {
            return;
        }
        let mut i = n - 1;
        while i > 0 {
            let j = self.below((i + 1) as u64) as usize;
            slice.swap(i, j);
            i -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(1234);
        let mut b = Rng::new(1234);
        for _ in 0..256 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn fork_is_order_and_consumption_independent() {
        let root = Rng::new(7);
        let a1 = root.fork("a").key();
        let b1 = root.fork("b").key();
        // Opposite order → identical keys.
        assert_eq!(root.fork("b").key(), b1);
        assert_eq!(root.fork("a").key(), a1);
        assert_ne!(a1, b1);
        // Draining the parent does not change what its children become.
        let mut drained = root.clone();
        for _ in 0..1000 {
            drained.next_u64();
        }
        assert_eq!(drained.fork("a").key(), a1);
    }

    #[test]
    fn below_is_bounded() {
        let mut rng = Rng::new(9);
        for _ in 0..10_000 {
            assert!(rng.below(7) < 7);
        }
        assert_eq!(rng.below(1), 0);
    }
}
