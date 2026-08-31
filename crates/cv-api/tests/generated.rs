//! The generated table agrees with the manifest it came from.
//!
//! ⚠ This is the test that makes "generated" mean something. Without it, `cv-api/src/lib.rs` is just
//! a committed file that *looks* generated, and a hand edit would survive until someone happened to
//! run the generator. Cross-checking against `cv-manifest`'s own parse of the same source is what
//! turns the claim into a check.
//!
//! `cargo xtask check` covers the byte-level question — is the file current. This covers the
//! semantic one — does it say what the manifest says.

use cv_api::{
    ancestors, find, DeclKind, Status, CLASSES, ENUM_COUNT, MEMBER_COUNT, OBJECT_COUNT,
    STRUCT_COUNT,
};

fn manifest() -> cv_manifest::Manifest {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../manifest/tier1.toml");
    let src = std::fs::read_to_string(path).expect("manifest/tier1.toml is readable");
    cv_manifest::parse(&src).expect("the manifest parses")
}

#[test]
fn counts_match_the_manifest() {
    let m = manifest();
    assert_eq!(CLASSES.len(), m.classes.len());
    assert_eq!(OBJECT_COUNT, m.count_of(cv_manifest::Kind::Object));
    assert_eq!(STRUCT_COUNT, m.count_of(cv_manifest::Kind::Struct));
    assert_eq!(ENUM_COUNT, m.count_of(cv_manifest::Kind::Enum));
    assert_eq!(MEMBER_COUNT, m.member_count());
}

#[test]
fn every_declaration_round_trips() {
    let m = manifest();
    for (src, gen) in m.classes.iter().zip(CLASSES.iter()) {
        assert_eq!(src.path, gen.path, "declaration order must be preserved");
        assert_eq!(src.extends.as_deref(), gen.extends);
        assert_eq!(src.sealed, gen.sealed);
        assert_eq!(src.doc, gen.doc);
        assert_eq!(src.fields.len(), gen.fields.len(), "{}", src.path);
        assert_eq!(src.methods.len(), gen.methods.len(), "{}", src.path);
        assert_eq!(src.values.len(), gen.values.len(), "{}", src.path);

        for (f, g) in src.fields.iter().zip(gen.fields.iter()) {
            assert_eq!(f.name, g.name);
            assert_eq!(f.ty, g.ty);
            assert_eq!(f.mutable, g.mutable);
            assert_eq!(f.exposed, g.exposed);
        }
        for (me, g) in src.methods.iter().zip(gen.methods.iter()) {
            assert_eq!(me.name, g.name);
            assert_eq!(me.returns, g.returns);
            assert_eq!(me.hook, g.hook);
            assert_eq!(me.params.len(), g.params.len(), "{}::{}", src.path, me.name);
            for (p, q) in me.params.iter().zip(g.params.iter()) {
                assert_eq!(p.name, q.name);
                assert_eq!(p.ty, q.ty);
            }
        }
    }
}

#[test]
fn lookup_and_ancestry_work() {
    let item = find("/Core/Item").expect("/Core/Item");
    let chain: Vec<_> = ancestors(item).iter().map(|c| c.path).collect();
    assert_eq!(chain, vec!["/Core/Actor", "/Core/Object"]);
    assert_eq!(item.short_name(), "Item");
    assert!(find("/Core/Nope").is_none());
}

/// The hook set is what a schematic's OVERRIDES list is built from, so it is worth asserting rather
/// than trusting: a hook that stops being a hook silently disappears from every override list.
#[test]
fn actor_hooks_are_marked() {
    let actor = find("/Core/Actor").expect("/Core/Actor");
    let hooks: Vec<&str> = actor.hooks().map(|h| h.name).collect();
    for expected in [
        "enables", "requires", "forbids", "judge", "gate", "harm", "grants",
    ] {
        assert!(hooks.contains(&expected), "`{expected}` must be a hook");
    }
    // Composition helpers are core-implemented and must not appear in an override list.
    for not_a_hook in ["component", "add_component", "world_transform"] {
        assert!(
            !hooks.contains(&not_a_hook),
            "`{not_a_hook}` is not a question the core asks"
        );
    }
}

/// Only `Stable` reaches a palette. A `Proposed` member exists here and in the reference, but content
/// must not be able to depend on it.
#[test]
fn proposed_members_are_still_marked() {
    let trav = find("/Core/TraversalComponent").expect("/Core/TraversalComponent");
    let clearance = trav
        .methods
        .iter()
        .find(|m| m.name == "clearance")
        .expect("clearance");
    assert_eq!(clearance.status, Status::Proposed);
}

/// Structs are copied, not subclassed — so none of them may declare an ancestor.
#[test]
fn structs_do_not_extend() {
    for c in CLASSES.iter().filter(|c| c.kind == DeclKind::Struct) {
        assert!(
            c.extends.is_none(),
            "{} is a struct and must not extend",
            c.path
        );
    }
}
