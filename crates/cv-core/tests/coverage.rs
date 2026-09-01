//! **The coverage ratchet** — every tier-1 declaration is either built or explicitly owed.
//!
//! # Why this test exists
//!
//! The design enumerates a surface; the plan is supposed to carry every item of it to a milestone; the
//! code is supposed to implement every milestone. Each hop was checked by **reading**, and reading
//! cannot establish absence — it establishes only that nothing caught the reader's eye.
//!
//! It failed exactly that way. `CV_*` metadata is named in two design documents, with its six keys
//! enumerated in one of them, and **no milestone ever picked it up**. Nothing was wrong in any file;
//! the item simply fell between two documents, and every subsequent read of either one looked fine.
//!
//! ⚠ **So the check is not "did someone look".** Every declaration in `manifest/tier1.toml` — which is
//! itself checked against the design, member for member — must appear below in exactly one of two
//! lists. Adding a declaration and building nothing fails this test; adding a declaration and *not
//! deciding* fails it too, because the compiler will not let a name sit in neither list.
//!
//! # What "built" means here
//!
//! That a Rust name exists which implements it. It does **not** mean conformant — conformance is what
//! the per-milestone tests are for. This test answers one narrower question that nothing else asked:
//! *is there anything the tier-1 surface promises that nobody has taken responsibility for?*

use std::collections::BTreeSet;

/// Declarations with a Rust counterpart today.
///
/// ⚠ The name here is the **manifest** name. Where the Rust spelling differs, the mapping is recorded
/// in [`ALIASES`] rather than left to a reader to infer.
const BUILT: &[&str] = &[
    // --- object model ---
    "Object",
    "Actor",
    "Item",
    "Component",
    "Surface",
    "Resource",
    "Budget",
    "BudgetBook",
    "MeshComponent",
    "ShapeComponent",
    "MountComponent",
    "TraversalComponent",
    "CheckpointComponent",
    "FastTravelComponent",
    "StateSetterComponent",
    "BlocksTraversalComponent",
    // --- geometry ---
    "Shape",
    "SolidShape",
    "SurfaceShape",
    "CompositeShape",
    "CollisionBody",
    "CubeShape",
    "SphereShape",
    "HemisphereShape",
    "ConeShape",
    "CapsuleShape",
    "CylinderShape",
    "PrismShape",
    "TorusShape",
    "PipeShape",
    "ArchShape",
    "RampShape",
    "StairsShape",
    "SpiralStairsShape",
    "QuadShape",
    "TriangleShape",
    "DiscShape",
    "EllipseShape",
    // --- rules and verdicts ---
    "Rule",
    "AlwaysRule",
    "NeverRule",
    "HoldsRule",
    "HasComponentRule",
    "AllOfRule",
    "AnyOfRule",
    "NegateRule",
    "NearbyRule",
    "Verdict",
    "AcceptedVerdict",
    "OverBudgetVerdict",
    "BlockedVerdict",
    "UnsuitableVerdict",
    // --- routing, needs, scheduling ---
    "Route",
    "Path",
    "PathStep",
    "Constraint",
    "AloneInScope",
    "MinDistanceFrom",
    "MaxDistanceFrom",
    "MountedOn",
    "WithinScope",
    "NotWithinScope",
    "Cohort",
    "SpherePin",
    "ScheduleRule",
    "PlacedAfter",
    "ExclusiveWith",
    "Supersedes",
    "Preference",
    "Rationale",
    // --- values ---
    "Vec3",
    "Mat4",
    "Transform",
    "Aabb",
    "CollisionData",
    "Span",
    "Dial",
    "Quantity",
    "Curve",
    "Harm",
    "Support",
    "Occupant",
    "Trivalent",
    "Hit",
    "Cost",
    "DistanceCost",
    "TimeCost",
    "PoolCost",
    "BudgetRef",
    "NamedBudget",
    "InlineBudget",
    "AdaptiveRange",
    "Approach",
    "DirectionCone",
    "Tag",
    "TagQuery",
    "Kind",
    "Ref",
    "Unlock",
    "MetaValue",
    "BoolMeta",
    "IntMeta",
    "FloatMeta",
    "StringMeta",
    "Vec3Meta",
    "TransformMeta",
    "ArrayMeta",
    "MapMeta",
    "RefMeta",
    // --- enums ---
    "Face",
    "Role",
    "ItemClass",
    "CollisionLayer",
    "CollisionMode",
    "Replenish",
    "InstanceScope",
    "Detail",
    "Fidelity",
    "Strictness",
    "Interpolation",
    // --- progression ---
    "ProgressionAxis",
    "Depth",
    "SpaceCount",
    "UnlockCount",
    "Sphere",
    "CurveTableResource",
    "UnlockTableResource",
    "Interaction",
    "Movement",
    "Displace",
    "RemoteUse",
    "Pool",
];

/// Declarations with **no** Rust counterpart yet, each naming who owes it.
///
/// ⚠ **A reason, not a bare name.** *"Not built"* with nothing beside it is how an item goes quiet for
/// six milestones; a named owner is what a future audit can check against the plan.
const OWED: &[(&str, &str)] = &[
    // --- the maths the design's tree names and cv-determinism does not have ---
    (
        "Vec2",
        "M26 L3 — no 2D surface work exists yet; the design's maths tree names it",
    ),
    ("Ray", "M26 L3 — raycast lands with the hull pass"),
    ("Plane", "M26 L3 — half-space tests land with the hull pass"),
    (
        "Quaternion",
        "ALIAS — cv-determinism spells it `Quat`; see ALIASES",
    ),
    // --- reporting surfaces ---
    ("Diagnostic", "M21 — the editor's lint and report channel"),
    ("Quota", "M10 — the solver's supply accounting"),
    (
        "ResolveState",
        "M10 — the solver's per-candidate resolution state",
    ),
    ("QueryFilter", "M12 — the VM's query dispatch"),
    ("BooleanOp", "M27 L4 — CSG on realized geometry"),
    // --- the pieces that need a file format or a VM first ---
    ("MeshResource", "M14 — mesh import"),
    (
        "Query",
        "M12 — the query builder is a Rust builder today, not a tier-1 object",
    ),
    ("ScopeHandle", "M10 — the solver's per-scope handle"),
    ("Exclusion", "M10 — push-it-out"),
    ("SkipPolicy", "M10 — per-lock skip policy"),
    ("PlacementNeed", "M10 — requires() returns these"),
    ("NeedsActor", "M10 — a PlacementNeed form"),
    ("NeedsClearance", "M10 — a PlacementNeed form"),
    ("BlocksTraversal", "M10 — a PlacementNeed form"),
    (
        "Spine",
        "M11 — `.cvspine`; the format spec's `Kind'/Core/Spine'` base",
    ),
    ("SpineSlot", "M11 — `.cvspine`"),
    ("Rng", "cv-determinism owns it; not a cv-core type"),
    ("Context", "M12 — the VM supplies it"),
    (
        "Color",
        "M27 L4 — dressing; not yet declared in the manifest either",
    ),
];

/// Where the Rust spelling differs from the manifest's.
///
/// ⚠ **Recorded rather than inferred.** A reader who greps for `Quaternion` and finds nothing should
/// land here, not conclude it is missing — which is exactly the wrong conclusion an audit would draw.
const ALIASES: &[(&str, &str)] = &[("Quaternion", "cv_determinism::Quat")];

fn manifest_paths() -> Vec<String> {
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../manifest/tier1.toml"),
    )
    .expect("the manifest is readable");
    src.lines()
        .filter_map(|l| l.strip_prefix("path = \"/Core/"))
        .filter_map(|l| l.strip_suffix('"'))
        .map(str::to_string)
        .collect()
}

#[test]
fn every_declaration_is_either_built_or_explicitly_owed() {
    // ⚠ **The ratchet.** A new manifest declaration cannot pass this without someone deciding which
    // list it belongs in — which is the decision that never happened for `CV_*` metadata.
    let built: BTreeSet<&str> = BUILT.iter().copied().collect();
    let owed: BTreeSet<&str> = OWED.iter().map(|(n, _)| *n).collect();

    let mut undecided = Vec::new();
    for path in manifest_paths() {
        if !built.contains(path.as_str()) && !owed.contains(path.as_str()) {
            undecided.push(path);
        }
    }
    assert!(
        undecided.is_empty(),
        "tier-1 declarations nobody has taken responsibility for:\n  {}\n\
         Add each to BUILT (it exists) or to OWED (with the milestone that owes it).",
        undecided.join("\n  ")
    );
}

#[test]
fn nothing_is_in_both_lists() {
    // Being in both means one of them is a lie, and a reader cannot tell which.
    let built: BTreeSet<&str> = BUILT.iter().copied().collect();
    let both: Vec<&str> = OWED
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| built.contains(n))
        .filter(|n| !ALIASES.iter().any(|(a, _)| a == n))
        .collect();
    assert!(both.is_empty(), "claimed both built and owed: {both:?}");
}

#[test]
fn every_owed_item_names_who_owes_it() {
    // ⚠ *"Not built"* with nothing beside it is how an item goes quiet for six milestones.
    for (name, reason) in OWED {
        assert!(
            reason.len() > 12 && reason.contains(char::is_whitespace),
            "{name} is owed by nobody in particular: {reason:?}"
        );
    }
}

#[test]
fn the_lists_name_only_things_the_manifest_declares() {
    // A stale entry is worse than no entry: it reports coverage of something that no longer exists.
    let declared: BTreeSet<String> = manifest_paths().into_iter().collect();
    // These are legitimately outside the manifest and named here for the reader's sake.
    let external: BTreeSet<&str> = [
        "Rng",
        "Context",
        "Color",
        "Spine",
        "SpineSlot",
        "Query",
        "PlacementNeed",
        "ScopeHandle",
        "MeshResource",
        "Exclusion",
        "SkipPolicy",
        "BlocksTraversal",
        "NeedsActor",
        "NeedsClearance",
    ]
    .into_iter()
    .collect();

    let mut stale = Vec::new();
    for name in BUILT.iter().chain(OWED.iter().map(|(n, _)| n)) {
        if !declared.contains(*name) && !external.contains(name) {
            stale.push(*name);
        }
    }
    assert!(stale.is_empty(), "listed but no longer declared: {stale:?}");
}

#[test]
fn the_core_writes_every_cv_key_the_design_promises() {
    // ⚠ **The specific miss this ratchet was built after.** `11-host.md` §7 enumerates six keys the
    // descriptor carries; nothing checked that the core wrote any of them, and it wrote none.
    use cv_core::handoff::keys;
    assert_eq!(
        keys::ALL.to_vec(),
        vec![
            "CV_role",
            "CV_layer",
            "CV_sphere",
            "CV_seed_path",
            "CV_grants",
            "CV_ambient"
        ],
        "the design's enumerated payload: role, layer, sphere, seed path, grants, ambient flags"
    );
}
