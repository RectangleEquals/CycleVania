//! Stable, target-independent hashing.
//!
//! `std`'s `DefaultHasher` is explicitly **not** stable across releases and is randomly seeded per
//! process, so it can never appear anywhere in generation. These are fixed algorithms over raw bytes:
//! the same input yields the same output on every target, every build, forever. They are the basis for
//! RNG stream forking ([`crate::Rng::fork`]) and content-derived object identity.

/// FNV-1a, 64-bit. Fixed algorithm over raw bytes — stable across targets and releases.
///
/// Chosen for being tiny, dependency-free, and byte-order explicit. It is *not* cryptographic and is
/// not collision-resistant against an adversary; it is a determinism primitive, not a security one.
#[inline]
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xCBF2_9CE4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    let mut h = OFFSET_BASIS;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// FNV-1a over a string's UTF-8 bytes.
#[inline]
pub fn fnv1a_str(s: &str) -> u64 {
    fnv1a_64(s.as_bytes())
}

/// SplitMix64's finalizer — a strong 64-bit avalanche mix.
///
/// Turns a poorly-distributed integer (a counter, a low-entropy id) into one whose bits are well
/// spread. Bijective, so it never introduces collisions.
#[inline]
pub fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Combine two hashes order-dependently, for deriving a child identity from a parent plus a label.
#[inline]
pub fn combine(a: u64, b: u64) -> u64 {
    mix64(a ^ mix64(b))
}

/// A stable content digest over a byte sequence — FNV-1a with an avalanche finalizer.
///
/// FNV alone leaves near-identical inputs in nearby buckets; the [`mix64`] pass spreads them, which
/// matters when a digest is *compared for equality* to decide whether two builds are the same recipe.
/// Like everything here it is a determinism primitive, not a cryptographic one: it detects accidental
/// difference, not deliberate forgery.
#[inline]
pub fn digest64(bytes: &[u8]) -> u64 {
    mix64(fnv1a_64(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_is_stable() {
        // Pinned values — a change here is a determinism break, not a refactor.
        assert_eq!(fnv1a_64(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(fnv1a_str("a"), 0xAF63_DC4C_8601_EC8C);
        assert_eq!(fnv1a_str("foobar"), 0x85944171F73967E8);
    }

    #[test]
    fn distinct_inputs_differ() {
        assert_ne!(fnv1a_str("enemies"), fnv1a_str("items"));
        assert_ne!(mix64(0), mix64(1));
        assert_ne!(
            combine(1, 2),
            combine(2, 1),
            "combine must be order-dependent"
        );
    }

    #[test]
    fn mix64_is_bijective_on_samples() {
        let mut seen = std::collections::HashSet::new();
        for i in 0..10_000u64 {
            assert!(seen.insert(mix64(i)), "mix64 collided at {i}");
        }
    }
}
