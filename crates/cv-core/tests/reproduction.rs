//! M05 exit criteria, demonstrated end-to-end rather than asserted about hashes in isolation.
//!
//! A toy generator stands in for the real pipeline: it takes a registry and a seed and produces a
//! world. That is enough to show the three properties reproducibility actually rests on:
//!
//! * **same fingerprint + same seed ⇒ identical world**
//! * **changing the recipe** (content, config, core version) **changes the fingerprint**
//! * **changing only the seed does not** — different worlds, one recipe
//!
//! The last is the one that is easy to get wrong and expensive to discover late. If the seed leaked
//! into the fingerprint, every world would be its own "build", and the question a bug report needs to
//! answer — *"can my build reproduce yours?"* — would become unanswerable.

use cv_core::serialize::{from_bytes, to_bytes};
use cv_core::{
    ContentKind, ContentRegistry, Fingerprint, FingerprintBuilder, NodeGraph, NodeKind,
    ReproductionBundle, ReproductionError,
};
use cv_determinism::Rng;

/// The content a world may be built from.
fn registry() -> ContentRegistry {
    let mut r = ContentRegistry::new();
    r.register(ContentKind::Actor, "crawler/door_heavy", 0x1001)
        .unwrap();
    r.register(ContentKind::Item, "crawler/key_bronze", 0x1002)
        .unwrap();
    r.register(ContentKind::Token, "blink_dash", 0x1003)
        .unwrap();
    r.register(ContentKind::Puzzle, "crawler/gate_relay", 0x1004)
        .unwrap();
    r.register(ContentKind::Component, "hinge", 0x1005).unwrap();
    r
}

fn recipe_of(registry: &ContentRegistry, core_version: &str, scale: f64) -> Fingerprint {
    FingerprintBuilder::new(core_version)
        .content(registry)
        .config_f64("worldScale", scale)
        .config_u64("reachTarget", 4)
        .finish()
}

/// A stand-in for the pipeline: deterministic structural choices driven entirely by the seed.
///
/// It uses the schedulable content only, which is what L1 will do — a `Component` is not something the
/// algorithm places on its own.
fn generate(registry: &ContentRegistry, seed: u64) -> NodeGraph {
    let mut g = NodeGraph::new(1.0, seed);
    let rng = Rng::new(seed);
    let placeable: Vec<String> = registry
        .schedulable()
        .map(|(_, e)| e.path().to_string())
        .collect();

    let mut layout = rng.fork("layout");
    let reach_count = layout.range_u64(2, 5) as usize;

    for r in 0..reach_count {
        let reach = g.add_child(g.root(), format!("reach_{r}")).unwrap();
        let area = g.add_child(reach, format!("area_{r}")).unwrap();

        let mut per_area = rng.fork("area").fork_index(r as u64);
        let space_count = per_area.range_u64(1, 4) as usize;
        let mut spaces = Vec::new();
        for s in 0..space_count {
            // Name each Space after a piece of schedulable content, so the registry's contents
            // genuinely influence the output.
            let pick = &placeable[per_area.below(placeable.len() as u64) as usize];
            spaces.push(g.add_child(area, format!("space_{r}_{s}:{pick}")).unwrap());
        }
        for w in spaces.windows(2) {
            g.connect(w[0], w[1]).unwrap();
        }
    }
    debug_assert!(g.check_invariants().is_none());
    g
}

#[test]
fn same_fingerprint_and_seed_produce_an_identical_world() {
    let reg = registry();
    let recipe = recipe_of(&reg, "0.1.0", 1.0);

    let first = generate(&reg, 12_345);
    let second = generate(&reg, 12_345);
    assert_eq!(first, second);
    assert_eq!(
        to_bytes(&first),
        to_bytes(&second),
        "byte-identical, not merely equal"
    );

    // And the bundle verifies the regeneration.
    let bundle =
        ReproductionBundle::new(recipe, 12_345).with_output(ReproductionBundle::digest_of(&first));
    assert!(bundle.verify(recipe, &second).is_ok());
}

#[test]
fn changing_only_the_seed_gives_a_different_world_under_the_same_recipe() {
    let reg = registry();
    let recipe = recipe_of(&reg, "0.1.0", 1.0);

    let a = generate(&reg, 1);
    let b = generate(&reg, 2);
    assert_ne!(
        to_bytes(&a),
        to_bytes(&b),
        "different seeds should explore different worlds"
    );

    // The recipe is unchanged — this is the property the whole split exists for.
    let bundle_a = ReproductionBundle::new(recipe, 1);
    let bundle_b = ReproductionBundle::new(recipe, 2);
    assert_eq!(bundle_a.fingerprint, bundle_b.fingerprint);
    assert!(bundle_a.check(recipe).is_ok());
    assert!(bundle_b.check(recipe).is_ok());
}

#[test]
fn changing_the_recipe_changes_the_fingerprint() {
    let reg = registry();
    let original = recipe_of(&reg, "0.1.0", 1.0);

    // A newer core.
    assert_ne!(original, recipe_of(&reg, "0.2.0", 1.0));
    // A different world scale.
    assert_ne!(original, recipe_of(&reg, "0.1.0", 2.0));

    // Content added.
    let mut extended = registry();
    extended
        .register(ContentKind::Spine, "spines/ascent", 0x2001)
        .unwrap();
    assert_ne!(original, recipe_of(&extended, "0.1.0", 1.0));

    // Content whose *source* changed — same declaration, different behaviour.
    let mut rebuilt = ContentRegistry::new();
    for (_, e) in reg.iter() {
        let digest = if e.path() == "blink_dash" {
            0xBEEF
        } else {
            e.source_digest()
        };
        rebuilt.register(e.kind(), e.path(), digest).unwrap();
    }
    assert_ne!(
        original,
        recipe_of(&rebuilt, "0.1.0", 1.0),
        "a script change must invalidate reproduction even though the content list is identical"
    );
}

#[test]
fn a_recipe_change_is_reported_before_anyone_compares_worlds() {
    let reg = registry();
    let theirs = recipe_of(&reg, "0.1.0", 1.0);
    let world = generate(&reg, 99);
    let bundle =
        ReproductionBundle::new(theirs, 99).with_output(ReproductionBundle::digest_of(&world));

    // Our build has newer content.
    let mut ours_reg = registry();
    ours_reg
        .register(ContentKind::Actor, "crawler/door_light", 0x3001)
        .unwrap();
    let ours = recipe_of(&ours_reg, "0.1.0", 1.0);

    let err = bundle.verify(ours, &world).unwrap_err();
    assert!(
        matches!(err, ReproductionError::FingerprintMismatch { .. }),
        "a build difference must not be reported as a determinism bug"
    );
    assert!(err.to_string().contains("cannot reproduce that world"));
}

#[test]
fn the_registry_actually_influences_the_world() {
    // Guards against the fingerprint being sensitive to content while generation quietly is not —
    // which would make the fingerprint stricter than reality, but is worth knowing either way.
    let reg = registry();
    let mut swapped = ContentRegistry::new();
    swapped
        .register(ContentKind::Actor, "other/door", 0x1001)
        .unwrap();
    swapped
        .register(ContentKind::Item, "other/key", 0x1002)
        .unwrap();
    swapped
        .register(ContentKind::Token, "other_dash", 0x1003)
        .unwrap();
    swapped
        .register(ContentKind::Puzzle, "crawler/other_relay", 0x1004)
        .unwrap();

    assert_ne!(
        to_bytes(&generate(&reg, 7)),
        to_bytes(&generate(&swapped, 7))
    );
}

#[test]
fn non_schedulable_content_is_never_placed() {
    // The `hinge` Component is registered but must not appear in a generated world: L1 draws from the
    // schedulable set only.
    let reg = registry();
    let g = generate(&reg, 4_242);
    let placed_a_component = g
        .of_kind(NodeKind::Space)
        .any(|(_, n)| cv_core::Object::name(n).contains("hinge"));
    assert!(
        !placed_a_component,
        "a Component is not schedulable and must not be placed"
    );
    assert_eq!(
        reg.schedulable().count(),
        4,
        "door, key, blink_dash, caverns"
    );
}

#[test]
fn a_bundle_survives_being_written_and_read_back() {
    let reg = registry();
    let recipe = recipe_of(&reg, "0.1.0", 1.0);
    let world = generate(&reg, 0xFEED);
    let bundle =
        ReproductionBundle::new(recipe, 0xFEED).with_output(ReproductionBundle::digest_of(&world));

    // Round-trip the bundle *and* the registry, as a real reproduction artifact would.
    let bundle_back: ReproductionBundle = from_bytes(&to_bytes(&bundle)).unwrap();
    let reg_back: ContentRegistry = from_bytes(&to_bytes(&reg)).unwrap();

    assert_eq!(bundle_back, bundle);
    // The restored registry recomputes the same recipe...
    assert_eq!(recipe_of(&reg_back, "0.1.0", 1.0), recipe);
    // ...and regenerating from the restored inputs reproduces the world exactly.
    let regenerated = generate(&reg_back, bundle_back.seed);
    assert!(bundle_back
        .verify(recipe_of(&reg_back, "0.1.0", 1.0), &regenerated)
        .is_ok());
}
