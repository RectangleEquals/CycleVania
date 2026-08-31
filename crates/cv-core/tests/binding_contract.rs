//! The binding contract, checked rather than remembered.
//!
//! `.notes/Design/v0.2b/11-host.md` §2 lists rules the TypeScript seam imposes on the core. Most of
//! them are enforced by the manifest validator, because most of the seam is *generated* from the
//! manifest. These are the ones that live in hand-written core code, where nothing else would catch
//! a regression.
//!
//! # Why `ObjectId` gets its own section
//!
//! `ObjectId` wraps a `u64`, and `ObjectId::derived` is an FNV-1a hash — so the value is uniform
//! across the whole 64-bit range. A JavaScript integer is exact only below 2^53, which a uniform
//! 64-bit value clears **2047 times out of 2048**. If the raw value ever reached a host as a number,
//! roughly 99.95% of content-derived ids would corrupt silently.
//!
//! The internal `u64` is fine and deliberate: it is fast, `Copy`, and hashes well. What must never
//! happen is that value crossing the seam *as a number*. These tests pin the string form that
//! crosses instead.

use cv_core::object::{IdAllocator, ObjectId};

/// JavaScript's exact-integer ceiling. Above this, `Number` silently rounds.
const JS_SAFE_INTEGER: u64 = (1u64 << 53) - 1;

#[test]
fn the_canonical_form_is_a_string_that_cannot_be_read_as_a_number() {
    let id = ObjectId::derived("actor", "crawler/door_heavy");
    let s = id.to_string();

    // A leading `#` guarantees it, but assert the property rather than the prefix: the property is
    // what matters, and a future format change should have to reckon with it.
    assert!(
        s.parse::<f64>().is_err(),
        "`{s}` parses as a number, so a host could coerce it and lose precision"
    );
    assert!(
        s.parse::<u64>().is_err(),
        "`{s}` parses as an integer, so a host could coerce it and lose precision"
    );
}

#[test]
fn derived_ids_routinely_exceed_the_javascript_safe_range() {
    // Not a hypothetical: this is why the string form exists. If this test ever finds every id
    // comfortably under 2^53, the id scheme changed and the whole rule should be revisited.
    let unsafe_count = (0..256)
        .map(|i| ObjectId::derived("actor", &format!("room_{i}")))
        .filter(|id| id.to_raw() > JS_SAFE_INTEGER)
        .count();

    assert!(
        unsafe_count > 200,
        "only {unsafe_count}/256 derived ids exceed 2^53 — the hash is no longer full-width, and the \
         reason ObjectId must not cross the seam as a number needs re-checking"
    );
}

#[test]
fn the_canonical_form_is_stable_and_fixed_width() {
    // Fixed width matters for the editor and for trace diffs: a variable-length id makes two traces
    // of the same world misalign on a column that carries no meaning.
    for i in 0..64 {
        let s = ObjectId::derived("space", &format!("s{i}")).to_string();
        assert_eq!(s.len(), 17, "`{s}` is not the canonical width");
        assert!(s.starts_with('#'));
        assert!(
            s[1..]
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "`{s}` is not lowercase hex"
        );
    }
}

#[test]
fn derivation_is_content_addressed_and_allocation_is_not() {
    // The two id sources answer different questions, and conflating them would make a reproduction
    // bundle resolve "the object the trace complained about" to a different object.
    let a = ObjectId::derived("actor", "door");
    let b = ObjectId::derived("actor", "door");
    assert_eq!(
        a, b,
        "derived ids must not depend on when they were derived"
    );

    let mut alloc = IdAllocator::default();
    assert_ne!(
        alloc.allocate(),
        alloc.allocate(),
        "allocation is sequential"
    );
}

/// The reserved id must stay reserved, or "no object" becomes indistinguishable from an object.
#[test]
fn none_is_never_produced() {
    assert!(ObjectId::NONE.is_none());
    let mut alloc = IdAllocator::default();
    for _ in 0..64 {
        assert!(!alloc.allocate().is_none());
    }
    for i in 0..256 {
        assert!(!ObjectId::derived("x", &format!("{i}")).is_none());
    }
}

// ---------------------------------------------------------------------------------------------
// Deterministic iteration
// ---------------------------------------------------------------------------------------------

/// Hash-map iteration order is unspecified and varies per process. Anything whose order reaches the
/// output would make a world unreproducible in a way no seed could explain — so the core does not
/// use them at all, and this asserts that rather than trusting it.
///
/// ⚠ The arena's free list is a `Vec` for exactly this reason: slot reuse order must be
/// deterministic, and a `HashSet` of vacant indices would not be.
#[test]
fn the_core_contains_no_hash_containers() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut offenders = Vec::new();

    for entry in std::fs::read_dir(dir).expect("src is readable") {
        let path = entry.expect("a directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("a source file");
        for (i, line) in src.lines().enumerate() {
            let t = line.trim_start();
            // Prose about why they are absent is not a use of one.
            if t.starts_with("//") || t.starts_with("*") {
                continue;
            }
            if line.contains("HashMap") || line.contains("HashSet") {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                offenders.push(format!("{name}:{}: {}", i + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "hash-container iteration order is unspecified and would reach generated output:\n  {}\n\n\
         use an insertion-ordered container, or sort by key on serialize.",
        offenders.join("\n  ")
    );
}
