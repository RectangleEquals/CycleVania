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
    "SkipPolicy",
    // --- the solver's own surface ---
    "PlacementNeed",
    "Query",
    "SpineSlot",
    "Rng",
    "Context",
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
    // --- reporting surfaces ---
    ("Diagnostic", "M21 — the editor's lint and report channel"),
    ("Quota", "M10a — per-scope counts, which fill bands consume"),
    ("QueryFilter", "M12 — the VM's query dispatch"),
    ("BooleanOp", "M27 L4 — CSG on realized geometry"),
    // --- the pieces that need a file format or a VM first ---
    ("MeshResource", "M14 — mesh import"),
    (
        "ScopeHandle",
        "M12 — the Context lens's scope type, not a solver internal",
    ),
    ("Exclusion", "M10b — the adoption gate"),
    (
        "Spine",
        "M11 — `.cvspine`; the format spec's `Kind'/Core/Spine'` base",
    ),
];

/// Where the Rust spelling differs from the manifest's.
///
/// ⚠ **Recorded rather than inferred.** A reader who greps for `Quaternion` and finds nothing should
/// land here, not conclude it is missing — which is exactly the wrong conclusion an audit would draw.
const ALIASES: &[(&str, &str)] = &[
    ("Quaternion", "cv_determinism::Quat"),
    // The sealed Rule tree: Rust reads `Rule::Has`, the manifest declares `HoldsRule`.
    ("HoldsRule", "cv_core::mission::Rule::Has"),
    ("AllOfRule", "cv_core::mission::Rule::All"),
    ("AnyOfRule", "cv_core::mission::Rule::Any"),
    ("NegateRule", "cv_core::mission::Rule::Not"),
    // `String` is taken in Rust, so the metadata form is `Text`.
    ("StringMeta", "cv_core::meta::MetaValue::Text"),
    // A dial's *value* is the struct the design calls `Dial`; its identity is separate here.
    ("Dial", "cv_core::dial::DialValue"),
    // The `Resource` suffix marks a file-backed class in the manifest; the Rust type is the loaded
    // form, so it drops it.
    ("CurveTableResource", "cv_core::curve::CurveTable"),
    ("UnlockTableResource", "cv_core::unlock::UnlockTable"),
    // The `Needs` prefix marks a PlacementNeed form in the manifest; Rust reads them as variants.
    ("NeedsActor", "cv_core::need::PlacementNeed::Actor"),
    ("NeedsClearance", "cv_core::need::PlacementNeed::Clearance"),
    (
        "BlocksTraversal",
        "cv_core::need::PlacementNeed::BlocksTraversal",
    ),
    // The manifest names the node lifecycle `ResolveState`; Rust reads it as `NodeState`.
    //
    // ⚠ **Its OWED entry described a different thing entirely** — *"the solver's per-candidate
    // resolution state"* — and the manifest says `PROJECTED · RESERVED · REALIZED`, which is the
    // projected→reserved→realized lifecycle built at M03. A debt whose stated reason does not match
    // the surface it names is worse than a missing one: the milestone that honoured it would have
    // built a second, parallel state machine.
    ("ResolveState", "cv_core::node::NodeState"),
];

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

/// There are three dispositions, not two, and pretending there were two is what put an alias in
/// `OWED`. `Quaternion` sat there reading *"ALIAS - see ALIASES"*: a debt entry whose own text says
/// it is not a debt, parked in the only list that would satisfy the ratchet. An alias is its own
/// answer - *built, under a different spelling* - so it dispositions a declaration exactly as
/// `BUILT` does, and belongs in neither of the other two.
#[test]
fn every_declaration_has_a_disposition() {
    // ⚠ **The ratchet.** A new manifest declaration cannot pass this without someone deciding which
    // list it belongs in — which is the decision that never happened for `CV_*` metadata.
    let built: BTreeSet<&str> = BUILT.iter().copied().collect();
    let owed: BTreeSet<&str> = OWED.iter().map(|(n, _)| *n).collect();
    let aliased: BTreeSet<&str> = ALIASES.iter().map(|(n, _)| *n).collect();

    let mut undecided = Vec::new();
    for path in manifest_paths() {
        let p = path.as_str();
        if !built.contains(p) && !owed.contains(p) && !aliased.contains(p) {
            undecided.push(path);
        }
    }
    assert!(
        undecided.is_empty(),
        "tier-1 declarations nobody has taken responsibility for:\n  {}\n\
         Add each to BUILT (it exists), ALIASES (it exists under another name), or OWED
\n         (with the milestone that owes it).",
        undecided.join("\n  ")
    );
}

/// Every Rust type, enum, trait and variant name defined in the workspace.
///
/// ⚠ Variants are included because a design name legitimately maps onto one — `CubeShape` is
/// `Shape::Cube`. That looseness is right for confirming a `BUILT` claim and wrong for challenging an
/// `OWED` one, so [`rust_definitions`] is the stricter index the mirror check uses.
fn rust_names() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for dir in ["crates/cv-core/src", "crates/cv-determinism/src"] {
        let Ok(rd) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(src) = std::fs::read_to_string(e.path()) else {
                continue;
            };
            for line in src.lines() {
                let t = line.trim();
                for kw in ["pub struct ", "pub enum ", "pub trait ", "pub type "] {
                    if let Some(rest) = t.strip_prefix(kw) {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        if !name.is_empty() {
                            out.insert(name);
                        }
                    }
                }
                // Enum variants: a bare `Name` or `Name {`/`Name(` at variant indentation.
                if t.starts_with(|c: char| c.is_ascii_uppercase()) {
                    let name: String = t
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() && (t.ends_with(',') || t.contains('{') || t.contains('('))
                    {
                        out.insert(name);
                    }
                }
            }
        }
    }
    out
}

/// ⚠ **The `BUILT` list must be checkable, not assertable.**
///
/// It was populated by hand, and a hand-populated list of *"things that exist"* is the same failure as
/// a hand-populated audit one level up — it records what someone believed. This test makes each entry
/// prove itself against a real Rust definition.
#[test]
fn every_built_entry_names_something_that_actually_exists() {
    let names = rust_names();
    // A manifest name maps to a Rust name that drops the design's suffix — `CubeShape` is
    // `Shape::Cube`, `AlwaysRule` is `Rule::Always`, `AcceptedVerdict` is `Verdict::Accepted`.
    let suffixes = [
        "Shape",
        "Rule",
        "Verdict",
        "Cost",
        "Budget",
        "Meta",
        "Component",
    ];
    let mut phantom = Vec::new();
    for name in BUILT {
        if names.contains(*name) {
            continue;
        }
        let stripped = suffixes
            .iter()
            .filter_map(|s| name.strip_suffix(s))
            .any(|base| names.contains(base));
        // ⚠ A recorded alias counts. An *unrecorded* divergence does not — that is the whole point:
        // a name the design uses and the code does not is either a hole or a rename, and only the
        // person making the change knows which.
        let aliased = ALIASES.iter().any(|(a, _)| a == name);
        if !stripped && !aliased {
            phantom.push(*name);
        }
    }
    assert!(
        phantom.is_empty(),
        "listed as BUILT with no Rust definition anywhere:
  {}
         Either build it, or move it to OWED with the milestone that owes it.",
        phantom.join(
            "
  "
        )
    );
}

/// Every alias points at a Rust path whose leaf actually exists.
///
/// ⚠ Otherwise an alias becomes a way to *silence* the phantom check rather than answer it.
#[test]
fn every_alias_resolves_to_something_real() {
    let names = rust_names();
    let mut dangling = Vec::new();
    for (design_name, rust_path) in ALIASES {
        let leaf = rust_path.rsplit("::").next().unwrap_or(rust_path);
        if !names.contains(leaf) {
            dangling.push(format!("{design_name} -> {rust_path}"));
        }
    }
    assert!(
        dangling.is_empty(),
        "aliases pointing at nothing: {dangling:?}"
    );
}

/// Only genuine top-level definitions — no variants, no match arms.
///
/// ⚠ The loose index reported `Ray` and `Spine` as built when neither exists, because a `Ray {` in a
/// doc comment or a match arm reads the same as a declaration. A false *"you already built this"* is
/// the expensive direction: it retires a debt that is still owed.
fn rust_definitions() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for dir in ["crates/cv-core/src", "crates/cv-determinism/src"] {
        let Ok(rd) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(src) = std::fs::read_to_string(e.path()) else {
                continue;
            };
            for line in src.lines() {
                for kw in ["pub struct ", "pub enum ", "pub trait ", "pub type "] {
                    if let Some(rest) = line.trim().strip_prefix(kw) {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        if !name.is_empty() {
                            out.insert(name);
                        }
                    }
                }
            }
        }
    }
    out
}

/// ⚠ **The mirror of the phantom check, and the one that was missing.**
///
/// `every_built_entry_names_something_that_actually_exists` catches a `BUILT` entry that is not built.
/// Nothing caught an `OWED` entry that **is** — so a milestone could build something and leave the
/// ledger claiming a later milestone still owes it. That is a stale debt: it reads as work outstanding,
/// so a reader plans around a hole that closed.
#[test]
fn no_owed_entry_has_quietly_been_built() {
    let defined = rust_definitions();
    let mut done = Vec::new();
    for (name, _) in OWED {
        // ⚠ **An alias that resolves is *proof* of built, not an exemption from the check.** This read
        // `if aliased { continue }` — *"an alias is a recorded divergence, not a debt"* — which is true
        // of the divergence and false of the conclusion drawn from it. An alias says *this surface
        // exists in Rust under another spelling*, and `every_alias_resolves_to_something_real` proves
        // the other spelling is there. So the two lists are **contradictory**, and the exemption made
        // the contradiction unreachable: `NeedsActor` and `NeedsClearance` sat in both for a milestone.
        if let Some((_, target)) = ALIASES.iter().find(|(a, _)| a == name) {
            done.push(format!("{name} (aliased to {target}, which resolves)"));
            continue;
        }
        if defined.contains(*name) {
            done.push(format!("{name} (a Rust definition exists)"));
        }
    }
    assert!(
        done.is_empty(),
        "listed as OWED but already built:
  {}
         Move each to BUILT or ALIASES — a debt that is already paid reads as a hole that is
         still open, and a reader plans around it.",
        done.join(
            "
  "
        )
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

/// **The ledger ratchet**: the count of undispositioned design surfaces may not rise.
///
/// ⚠ **The ceiling is zero, and it got there by dispositioning the backlog rather than by narrowing
/// the extraction.** Every surface the design enumerates now has an answer: built, owed by a named
/// milestone, or refused in the design's own deliberately-absent table. A new design surface that
/// nobody answers for fails the build the moment it is written.
///
/// ⚠ **What this does *not* claim.** `owed` is satisfied by a milestone *naming* the surface, which
/// proves someone wrote it down — not that the milestone's plan for it is right. This test answers
/// *"has anyone taken responsibility?"*, and nothing more. Conformance is what the per-milestone tests
/// are for.
///
/// ⚠ **The ledger is gitignored, and so is this check.** Every row of it names a private design file
/// and a line number, which makes the *projection* exactly as private as the notes it projects — the
/// mistake was ever committing it. So the ratchet runs for whoever holds the design and is **inert
/// elsewhere**: a clone without `.notes/` has no ledger to check, and a test that failed there would
/// be demanding a file the repo deliberately does not carry.
#[test]
fn the_undispositioned_backlog_does_not_grow() {
    /// Lower this as surfaces are dispositioned. Never raise it.
    const CEILING: usize = 0;

    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.notes/Implementation/v0.2b/design-ledger.md");
    let Ok(src) = std::fs::read_to_string(&path) else {
        // No ledger, no design: nothing to ratchet against, and that is the expected state for anyone
        // who is not holding the private notes.
        return;
    };
    let line = src
        .lines()
        .find(|l| l.contains("none — undispositioned"))
        .expect("the ledger reports an undispositioned count");
    let count: usize = line
        .rsplit('|')
        .nth(1)
        .and_then(|c| c.trim().parse().ok())
        .expect("the count is a number");

    // ⚠ Written as a ceiling rather than `== 0`, and the lint is silenced rather than the code
    // changed: the comparison is only "absurd" because the backlog reached zero. An equality would
    // fail the day someone legitimately *lowers* a raised ceiling, which is the direction we want easy.
    #[allow(clippy::absurd_extreme_comparisons)]
    let within = count <= CEILING;
    assert!(
        within,
        "{count} design surfaces are undispositioned, up from {CEILING}.
         Either disposition the new one (name it in a milestone, build it, or refuse it in the
         design's deliberately-absent table) or say why the ceiling should move."
    );
}

#[test]
fn the_core_writes_every_cv_key_the_design_promises() {
    // ⚠ **The specific miss this ratchet was built after.** `11-host.md` §7 enumerates six keys the
    // descriptor carries; nothing checked that the core wrote any of them, and it wrote none.
    use cv_core::handoff::keys;
    assert_eq!(
        keys::ALL.to_vec(),
        vec![
            "CV_ROLE",
            "CV_LAYER",
            "CV_SPHERE",
            "CV_SEED_PATH",
            "CV_GRANTS",
            "CV_AMBIENT"
        ],
        "the design's enumerated payload: role, layer, sphere, seed path, grants, ambient flags"
    );
}

/// Every name this file tracks also has a row in the design ledger.
///
/// ⚠ **Two ratchets that do not know about each other are worse than one.** This file's domain is
/// `manifest/tier1.toml`; the ledger's is the design prose. Each was closed over its own domain and
/// neither could see the seam, so 37 declarations this file tracks — `MinDistanceFrom`, `MountedOn`,
/// `SpherePin`, `PlacedAfter`, `SkipPolicy`, `Detail`, `Fidelity` among them — had **no ledger row at
/// all**, while the ledger reported *"0 undispositioned"* over a total that silently omitted them. A
/// reader consulting either one got a number that was true and incomplete in a way nothing disclosed.
///
/// ⚠ **The ledger lives in the private notes and is absent from a clean checkout**, so this skips
/// rather than fails when it is not there — the same shape `xtask` uses. It is a seam check, not a
/// build dependency on files the public repo does not carry.
#[test]
fn every_tracked_name_has_a_ledger_row() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok(ledger) =
        std::fs::read_to_string(root.join(".notes/Implementation/v0.2b/design-ledger.md"))
    else {
        return;
    };

    let rows: BTreeSet<&str> = ledger
        .lines()
        .filter(|l| l.starts_with("| `"))
        .filter_map(|l| l.split('`').nth(1))
        .collect();

    // Named in this file for the reader's sake, and outside the manifest — so outside the ledger's
    // reach as well. Recorded, not silent.
    let outside: BTreeSet<&str> = ["Query", "Spine", "SpineSlot"].into_iter().collect();

    let mut absent: Vec<&str> = BUILT
        .iter()
        .copied()
        .chain(OWED.iter().map(|(n, _)| *n))
        .chain(ALIASES.iter().map(|(n, _)| *n))
        .filter(|n| !rows.contains(n) && !outside.contains(n))
        .collect();
    absent.sort_unstable();
    absent.dedup();

    assert!(
        absent.is_empty(),
        "tracked here but absent from the design ledger:
  {}
         Either the ledger's extraction cannot see them - widen it - or they belong in the
         `outside` list above with a reason.",
        absent.join(
            "
  "
        )
    );
}
