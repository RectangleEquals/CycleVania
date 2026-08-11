//! M01 determinism vectors + properties for the owned PRNG.
//!
//! Golden fixtures pin the exact byte output of the root stream and a fork tree, so any change to the
//! generator is caught. Property tests assert the forking discipline (order- and consumption-
//! independence) and that the distributions stay in range and unbiased where required.

mod common;
use common::assert_golden_bytes;
use cv_determinism::Rng;

/// Draw `n` u64s little-endian into a byte buffer.
fn draw_bytes(rng: &mut Rng, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * 8);
    for _ in 0..n {
        out.extend_from_slice(&rng.next_u64().to_le_bytes());
    }
    out
}

#[test]
fn root_stream_is_stable() {
    let mut rng = Rng::new(0x00C0_FFEE);
    let bytes = draw_bytes(&mut rng, 64);
    assert_golden_bytes("m01_root_seed_c0ffee.bin", &bytes);
}

#[test]
fn fork_tree_is_stable() {
    let root = Rng::new(42);
    let mut enemies = root.fork("enemies");
    let mut items = root.fork("items");
    let mut elite = enemies.fork("elite");
    let mut idx = root.fork_index(3);

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&draw_bytes(&mut enemies, 16));
    bytes.extend_from_slice(&draw_bytes(&mut items, 16));
    bytes.extend_from_slice(&draw_bytes(&mut elite, 16));
    bytes.extend_from_slice(&draw_bytes(&mut idx, 16));
    assert_golden_bytes("m01_fork_tree_seed42.bin", &bytes);
}

#[test]
fn sibling_forks_are_order_independent() {
    let root = Rng::new(7);
    // One order...
    let mut a1 = root.fork("a");
    let mut b1 = root.fork("b");
    // ...and the reverse.
    let mut b2 = root.fork("b");
    let mut a2 = root.fork("a");
    assert_eq!(draw_bytes(&mut a1, 8), draw_bytes(&mut a2, 8));
    assert_eq!(draw_bytes(&mut b1, 8), draw_bytes(&mut b2, 8));
}

#[test]
fn fork_is_consumption_independent() {
    let root = Rng::new(99);
    let mut child_early = root.fork("x");
    let mut drained = root.clone();
    for _ in 0..1000 {
        drained.next_u64();
    }
    let mut child_late = drained.fork("x");
    assert_eq!(
        draw_bytes(&mut child_early, 8),
        draw_bytes(&mut child_late, 8)
    );
}

#[test]
fn distinct_labels_give_distinct_streams() {
    let root = Rng::new(123);
    assert_ne!(root.fork("a").key(), root.fork("b").key());
    assert_ne!(root.fork_index(0).key(), root.fork_index(1).key());
    // The index space is domain-separated from the empty label.
    assert_ne!(root.fork("").key(), root.fork_index(0).key());
}

#[test]
fn distributions_stay_in_range() {
    let mut rng = Rng::new(0xABCD);
    for _ in 0..20_000 {
        assert!((0.0..1.0).contains(&rng.next_f64()));
        assert!((-3.0..5.0).contains(&rng.uniform(-3.0, 5.0)));
        assert!(rng.below(10) < 10);
        assert!((-5..5).contains(&rng.range_i64(-5, 5)));
        assert!((100..200).contains(&rng.range_u64(100, 200)));
        assert!((-2.0..2.0).contains(&rng.jitter(2.0)));
    }
}

#[test]
fn shuffle_is_a_permutation() {
    let mut rng = Rng::new(555);
    let mut v: Vec<u32> = (0..100).collect();
    rng.shuffle(&mut v);
    assert_ne!(
        v,
        (0..100).collect::<Vec<_>>(),
        "a 100-element shuffle should reorder"
    );
    v.sort_unstable();
    assert_eq!(
        v,
        (0..100).collect::<Vec<_>>(),
        "shuffle must preserve the multiset"
    );
}

#[test]
fn weighted_choice_ignores_zero_weights() {
    let mut rng = Rng::new(1);
    let w = [0.0, 1.0, 0.0];
    for _ in 0..1000 {
        assert_eq!(rng.weighted_choice(&w), 1);
    }
}

#[test]
fn weighted_choice_roughly_matches_weights() {
    let mut rng = Rng::new(2024);
    let weights = [1.0, 3.0]; // expect ~25% / ~75%
    let mut counts = [0u32; 2];
    let n = 100_000;
    for _ in 0..n {
        counts[rng.weighted_choice(&weights)] += 1;
    }
    let p1 = counts[1] as f64 / n as f64;
    assert!((0.72..0.78).contains(&p1), "weighted_choice skew: got {p1}");
}
