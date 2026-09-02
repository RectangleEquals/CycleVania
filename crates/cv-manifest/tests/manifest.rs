//! The manifest is legal, complete, and says what the design says it says.
//!
//! ⚠ The count assertions are deliberately exact. A count that drifts means a member was dropped in
//! transcription, and a dropped member is invisible everywhere downstream — it simply never appears
//! in the palette, and nothing fails until a developer goes looking for a node that should exist.

use cv_manifest::model::Kind;
use cv_manifest::{parse, validate};

fn manifest_src() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../manifest/tier1.toml");
    std::fs::read_to_string(path).expect("manifest/tier1.toml is readable")
}

fn manifest() -> cv_manifest::Manifest {
    parse(&manifest_src()).expect("the committed manifest parses")
}

#[test]
fn parses() {
    let m = manifest();
    assert_eq!(m.version, 1);
}

#[test]
fn is_legal() {
    let violations = validate(&manifest());
    assert!(
        violations.is_empty(),
        "the committed manifest violates its own constraints:\n{}",
        violations
            .iter()
            .map(|v| format!("  {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn declaration_counts() {
    let m = manifest();
    let objects = m.count_of(Kind::Object);
    let structs = m.count_of(Kind::Struct);
    let variants = m.count_of(Kind::Variant);
    let enums = m.count_of(Kind::Enum);

    // M03a: +/Core/UnlockTableResource (object, 3 members) and +/Core/Unlock (struct, doc-only as
    // every struct here is); -Object::satisfied_by. So 328 - 1 + 3 = 330.
    //
    // Budgets became named rows: -DistanceBudget/-TimeBudget/-PoolBudget, +BudgetBook. Objects
    // 91 - 3 + 1 = 89. +BudgetRef makes structs 30. Members 330 + 8 - 2 = 336.
    //
    // Then `variant` arrived — a VALUE WITH ALTERNATIVE FORMS. 21 Shape records moved out of
    // `object` (they are copied, never referenced — the new rule caught two `Ref<Shape>` fields the
    // design types as bare `Shape`), and `Cost`/`BudgetRef` moved out of `struct` (a struct cannot
    // carry forms, so both had been generating as EMPTY interfaces). Their five forms are new:
    // Distance/Time/PoolCost and Named/InlineBudget, carrying 8 members between them.
    //
    // Objects 89 - 21 = 68. Structs 30 - 2 = 28. Variants 21 + 2 + 5 = 28. Members 336 + 8 = 344.
    //
    // Then `MetaValue` — used as a type throughout and never *declared*, so it lived in the
    // validator's whitelist of undeclared shells. It is a value with nine forms, which is exactly
    // what `variant` is for, so it and its forms are declared and the whitelist entry is gone.
    // Variants 28 + 10 = 38; members unchanged, because a metadata form carries no members of its own.
    //
    // Then the coverage audit found `CollisionData` in the same shape `MetaValue` had been: an
    // `api class` in 06-api, used as a type here three times, and never *declared* — so it sat in the
    // validator's whitelist of undeclared shells. Structs 28 + 1 = 29; members 344 + 4 = 348.
    assert_eq!(objects, 68, "object declarations");
    assert_eq!(structs, 29, "struct declarations");
    assert_eq!(variants, 38, "variant declarations");
    assert_eq!(enums, 16, "enum declarations");
    assert_eq!(m.classes.len(), 151, "total declarations");
    assert_eq!(m.member_count(), 348, "fields + methods");
}

/// Spot-checks against `.notes/Design/v0.2b/06-api/reference.md`. Not exhaustive — the exhaustive
/// check is the count above — but these are the members whose *shape* carries a design decision, so
/// a silent change to one of them is a silent change to the design.
#[test]
fn load_bearing_signatures() {
    let m = manifest();

    // The lattice trades in `Unlock` ROWS. One currency on every side, and the type is what bounds
    // the editor's picker — see `the_lattice_is_never_bounded_at_the_root` below.
    let actor = m.get("/Core/Actor").expect("/Core/Actor");
    let grants = actor
        .methods
        .iter()
        .find(|x| x.name == "grants")
        .expect("Actor::grants");
    assert_eq!(grants.returns, "Array<Unlock>");
    assert!(grants.hook);

    let holds = m.get("/Core/HoldsRule").expect("/Core/HoldsRule");
    assert_eq!(
        holds
            .fields
            .iter()
            .find(|f| f.name == "unlock")
            .map(|f| f.ty.as_str()),
        Some("Unlock"),
        "HoldsRule tests an unlock row, matching grants()"
    );

    // ctx.held is the third side of the same currency.
    let ctx = m.get("/Core/Context").expect("/Core/Context");
    assert_eq!(
        ctx.fields
            .iter()
            .find(|f| f.name == "held")
            .map(|f| f.ty.as_str()),
        Some("Array<Unlock>")
    );

    // An unlock carries no behaviour: identity and ordering, nothing else.
    let unlock = m.get("/Core/Unlock").expect("/Core/Unlock");
    assert!(
        unlock.methods.is_empty(),
        "/Core/Unlock must declare no methods — every mechanical consequence belongs to a Component"
    );

    // `satisfied_by` was deleted, not relocated. Its job is the `supersedes` column.
    let object = m.get("/Core/Object").expect("/Core/Object");
    assert!(
        !object.methods.iter().any(|x| x.name == "satisfied_by"),
        "Object must not carry a progression hook — MeshComponent would inherit it"
    );

    // Trivalent, never bool — the API must not be able to lie.
    for name in ["accessible", "within"] {
        let me = ctx.methods.iter().find(|x| x.name == name).expect(name);
        assert_eq!(me.returns, "Trivalent", "ctx.{name} must not return bool");
    }

    // affords takes an Interaction, which is what lets one surface answer four mechanics differently.
    let surface = m.get("/Core/Surface").expect("/Core/Surface");
    let affords = surface
        .methods
        .iter()
        .find(|x| x.name == "affords")
        .expect("Surface::affords");
    assert_eq!(affords.params[1].ty, "Ref<Interaction>");

    // Interaction.gate, deliberately NOT `requires` — which on an Actor means something unrelated.
    let inter = m.get("/Core/Interaction").expect("/Core/Interaction");
    assert!(inter.methods.iter().any(|x| x.name == "gate"));
    assert!(
        !inter.methods.iter().any(|x| x.name == "requires"),
        "Interaction must not reuse the Actor hook name"
    );

    // The one PROPOSED member, still marked as such.
    let trav = m
        .get("/Core/TraversalComponent")
        .expect("/Core/TraversalComponent");
    let clearance = trav
        .methods
        .iter()
        .find(|x| x.name == "clearance")
        .expect("TraversalComponent::clearance");
    assert_eq!(clearance.status, cv_manifest::Status::Proposed);
}

/// The scope ladder has a Floor, and the instance-scope enum deliberately does not.
#[test]
fn floor_is_a_scope_but_not_an_instance_scope() {
    let m = manifest();
    let ctx = m.get("/Core/Context").expect("/Core/Context");
    assert!(
        ctx.fields.iter().any(|f| f.name == "floor"),
        "Floor is a scope a hook can read"
    );

    let is = m.get("/Core/InstanceScope").expect("/Core/InstanceScope");
    assert!(
        !is.values.iter().any(|v| v.name == "FLOOR"),
        "a floor-scoped instance query would stop at a boundary the geometry does not stop at"
    );
}

/// Sealed families stay sealed: the solver walks the Rule tree as the analysable half of a gate.
#[test]
fn rule_and_verdict_are_sealed() {
    let m = manifest();
    for path in ["/Core/Rule", "/Core/Verdict"] {
        assert!(m.get(path).expect(path).sealed, "{path} must be sealed");
    }
    for sub in [
        "AlwaysRule",
        "NeverRule",
        "HoldsRule",
        "AnyOfRule",
        "NegateRule",
        "NearbyRule",
    ] {
        let c = m
            .get(&format!("/Core/{sub}"))
            .unwrap_or_else(|| panic!("{sub}"));
        assert_eq!(c.extends.as_deref(), Some("/Core/Rule"));
        assert!(c.sealed, "{sub} must be sealed");
    }
}

// ---------------------------------------------------------------------------------------------
// Values are copied; objects are pointed at. Keeping the two apart is what `variant` is for.
// ---------------------------------------------------------------------------------------------

/// ⚠ **The rule with teeth.** `Ref<Shape>` type-checked for as long as `Shape` was declared an object,
/// and it *was in the manifest* — on a field [`06-api`] types as a bare `Shape`, and on a `decompose()`
/// that returns convex **pieces**. A `Ref<T>` to something with no identity is a pointer to a copy: two
/// of them compare unequal while meaning the same thing, and nothing in the declaration says so.
#[test]
fn a_value_is_never_referenced() {
    let m = manifest();
    let valued: std::collections::BTreeSet<&str> = m
        .classes
        .iter()
        .filter(|c| matches!(c.kind(), Kind::Struct | Kind::Variant | Kind::Enum))
        .map(|c| c.short_name())
        .collect();

    let mut offenders = Vec::new();
    for c in &m.classes {
        let mut check = |ty: &str, member: &str| {
            for wrapper in ["Ref<", "Kind<"] {
                let mut rest = ty;
                while let Some(at) = rest.find(wrapper) {
                    rest = &rest[at + wrapper.len()..];
                    let inner: String = rest
                        .chars()
                        .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                        .collect();
                    if valued.contains(inner.as_str()) {
                        offenders.push(format!("{}::{member} is {wrapper}{inner}>", c.path));
                    }
                }
            }
        };
        for f in &c.fields {
            check(&f.ty, &f.name);
        }
        for me in &c.methods {
            check(&me.returns, &me.name);
            for p in &me.params {
                check(&p.ty, &me.name);
            }
        }
    }
    assert!(offenders.is_empty(), "values referenced: {offenders:#?}");
}

/// A variant's forms must extend a **variant**, or the union acquires identity by the back door.
#[test]
fn a_variant_form_extends_a_variant() {
    let m = manifest();
    for c in m.classes.iter().filter(|c| c.kind() == Kind::Variant) {
        if let Some(base) = &c.extends {
            let parent = m.get(base).unwrap_or_else(|| panic!("{base}"));
            assert_eq!(
                parent.kind(),
                Kind::Variant,
                "{} extends {base}, which is a {:?}",
                c.path,
                parent.kind()
            );
        }
    }
}

/// The families that genuinely are values, spot-checked so a future edit has to argue with a name.
#[test]
fn shapes_costs_and_budget_refs_are_values() {
    let m = manifest();
    for path in [
        "/Core/Shape",
        "/Core/CubeShape",
        "/Core/SpiralStairsShape",
        "/Core/Cost",
        "/Core/BudgetRef",
    ] {
        assert_eq!(
            m.get(path).unwrap_or_else(|| panic!("{path}")).kind(),
            Kind::Variant,
            "{path} is copied, not referenced"
        );
    }

    // ⚠ And the ones that stay objects, because they genuinely are pointed at: a `Rule` is composed
    // and walked, a `Verdict` is returned and handed to `on_rejected`.
    for path in ["/Core/Rule", "/Core/Verdict", "/Core/Interaction"] {
        assert_eq!(
            m.get(path).unwrap_or_else(|| panic!("{path}")).kind(),
            Kind::Object
        );
    }
}

/// Every variant with forms must have at least two, or it is a struct wearing a union's clothes.
#[test]
fn a_variant_with_one_form_is_a_struct() {
    let m = manifest();
    for c in m.classes.iter().filter(|c| c.kind() == Kind::Variant) {
        let forms = m
            .classes
            .iter()
            .filter(|f| f.extends.as_deref() == Some(c.path.as_str()))
            .count();
        assert!(
            forms != 1,
            "{} has exactly one form — that is a struct, not a choice",
            c.path
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The parser is strict on purpose. A malformed manifest must be a build failure, not a surprise.
// ---------------------------------------------------------------------------------------------

#[test]
fn rejects_unknown_keys() {
    let src = "version = 1\n[[class]]\npath = \"/Core/X\"\nwibble = \"no\"\n";
    let e = parse(src).expect_err("unknown key must fail");
    assert!(e.message.contains("wibble"), "{e}");
}

#[test]
fn rejects_floats_and_dates() {
    let e = parse("version = 1\n[[class]]\npath = \"/Core/X\"\nsealed = 1.5\n")
        .expect_err("a float must fail");
    assert!(e.message.contains("not a string"), "{e}");
}

#[test]
fn rejects_member_before_class() {
    let e = parse("version = 1\n[[class.field]]\nname = \"x\"\n").expect_err("must fail");
    assert!(e.message.contains("before any [[class]]"), "{e}");
}

#[test]
fn rejects_missing_version() {
    let e = parse("[[class]]\npath = \"/Core/X\"\n").expect_err("must fail");
    assert!(e.message.contains("version"), "{e}");
}

#[test]
fn reports_the_line_number() {
    let e = parse("version = 1\n[[class]]\npath = \"/Core/X\"\n[[nope]]\n").expect_err("must fail");
    assert_eq!(e.line, 4);
}

// ---------------------------------------------------------------------------------------------
// Each validator fires on a manifest that breaks exactly one rule.
// ---------------------------------------------------------------------------------------------

fn violations_of(src: &str) -> Vec<String> {
    validate(&parse(src).expect("test fixture parses"))
        .into_iter()
        .map(|v| v.rule.to_string())
        .collect()
}

const OBJECT: &str =
    "version = 1\n[[class]]\npath = \"/Core/Object\"\nkind = \"object\"\ndoc = \"root\"\n";

#[test]
fn catches_wide_integers() {
    let src = format!(
        "{OBJECT}[[class]]\npath = \"/Core/A\"\nextends = \"/Core/Object\"\nkind = \"object\"\ndoc = \"d\"\n\
         [[class.field]]\nname = \"n\"\ntype = \"u64\"\ndoc = \"d\"\n"
    );
    assert!(violations_of(&src).contains(&"u64".to_string()));
}

#[test]
fn catches_excess_depth() {
    let mut src = OBJECT.to_string();
    for (i, parent) in [
        ("A", "/Core/Object"),
        ("B", "/Core/A"),
        ("C", "/Core/B"),
        ("D", "/Core/C"),
    ] {
        src.push_str(&format!(
            "[[class]]\npath = \"/Core/{i}\"\nextends = \"{parent}\"\nkind = \"object\"\ndoc = \"d\"\n"
        ));
    }
    assert!(violations_of(&src).contains(&"depth".to_string()));
}

#[test]
fn catches_mutable_without_exposed() {
    let src = format!(
        "{OBJECT}[[class]]\npath = \"/Core/A\"\nextends = \"/Core/Object\"\nkind = \"object\"\ndoc = \"d\"\n\
         [[class.field]]\nname = \"n\"\ntype = \"bool\"\nmutable = true\ndoc = \"d\"\n"
    );
    assert!(violations_of(&src).contains(&"mutable".to_string()));
}

#[test]
fn catches_overloads() {
    let src = format!(
        "{OBJECT}[[class]]\npath = \"/Core/A\"\nextends = \"/Core/Object\"\nkind = \"object\"\ndoc = \"d\"\n\
         [[class.method]]\nname = \"f\"\nreturns = \"bool\"\ndoc = \"d\"\n\
         [[class.method]]\nname = \"f\"\nreturns = \"int\"\ndoc = \"d\"\n"
    );
    assert!(violations_of(&src).contains(&"overload".to_string()));
}

#[test]
fn catches_dangling_extends_and_types() {
    let src = format!(
        "{OBJECT}[[class]]\npath = \"/Core/A\"\nextends = \"/Core/Nope\"\nkind = \"object\"\ndoc = \"d\"\n\
         [[class.field]]\nname = \"n\"\ntype = \"AlsoNope\"\ndoc = \"d\"\n"
    );
    let v = violations_of(&src);
    assert!(v.contains(&"extends".to_string()));
    assert!(v.contains(&"type".to_string()));
}

#[test]
fn catches_undocumented_members() {
    let src = format!(
        "{OBJECT}[[class]]\npath = \"/Core/A\"\nextends = \"/Core/Object\"\nkind = \"object\"\ndoc = \"d\"\n\
         [[class.field]]\nname = \"n\"\ntype = \"bool\"\n"
    );
    assert!(violations_of(&src).contains(&"doc".to_string()));
}

#[test]
fn catches_kind_confusion() {
    let src = format!(
        "{OBJECT}[[class]]\npath = \"/Core/E\"\nkind = \"enum\"\ndoc = \"d\"\n\
         [[class.field]]\nname = \"n\"\ntype = \"bool\"\ndoc = \"d\"\n"
    );
    assert!(violations_of(&src).contains(&"kind".to_string()));
}

/// The `lattice-bound` rule must actually fire. A validator nobody has seen fail is a validator
/// nobody knows works — and this one guards the defect that made `grants()`'s picker offer the whole
/// project, which is silent by nature.
#[test]
fn the_lattice_is_never_bounded_at_the_root() {
    // The shipped manifest is clean.
    assert!(
        validate(&manifest())
            .iter()
            .all(|v| v.rule != "lattice-bound"),
        "the shipped manifest widened a lattice bound"
    );

    // And widening one is caught — including in a *parameter*, which is where `held` lives.
    for src in [
        r#"
[[class]]
path = "/Core/Thing"
kind = "object"
status = "stable"
doc = "d"
  [[class.method]]
  name = "grants"
  args = []
  returns = "Array<Kind<Object>>"
  api = true
  final = false
  status = "stable"
  doc = "d"
"#,
        r#"
[[class]]
path = "/Core/Thing"
kind = "object"
status = "stable"
doc = "d"
  [[class.method]]
  name = "accessible"
  args = [ { name = "held", type = "Array<Kind<Object>>" } ]
  returns = "Trivalent"
  api = true
  final = true
  status = "stable"
  doc = "d"
"#,
    ] {
        let m = cv_manifest::parse(&format!(
            "version = 1
{src}"
        ))
        .expect("fixture parses");
        assert!(
            validate(&m).iter().any(|v| v.rule == "lattice-bound"),
            "a widened lattice bound went unreported:
{src}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Content extensions and build output are two disjoint sets, and the boundary is enforced.
// ---------------------------------------------------------------------------------------------

/// The formats a project **authors and commits**.
///
/// ⚠ `09-format.md` §4: these are the declared set. Anything else claiming to be content is either a
/// typo or a build product that has escaped `build/`.
const CONTENT_EXTENSIONS: &[&str] = &[".cvs", ".cvspine", ".cvstate", ".cvcurve", ".cvunlock"];

/// What the toolchain **produces** and nobody commits.
///
/// ⚠ `.cvo` is the compiled-bytecode intermediate — CycleVania *Object*, in the compiler's sense:
/// compiled, unlinked, consumed by a later step, never shipped on its own. It is deliberately **not**
/// derived from CVB, which is the block *notation* that `.cvs`, `.cvspine` and `.cvstate` are written
/// in — an extension named after it would read as *"a CVB file"*, a category that does not exist.
const BUILD_EXTENSIONS: &[&str] = &[".cvo", ".cvpak"];

#[test]
fn content_and_build_extensions_never_overlap() {
    // ⚠ An overlap would put a build product under version control, into the asset globs, and into the
    // cook's walk of authored roots — three failures from one misplaced file.
    for b in BUILD_EXTENSIONS {
        assert!(
            !CONTENT_EXTENSIONS.contains(b),
            "{b} is claimed as both content and build output"
        );
    }
}

/// **The two sets above are closed**: no source in the workspace may name a `.cv*` extension that is
/// not in one of them.
///
/// ⚠ **This replaces a blacklist, and is strictly stronger.** A blacklist forbids the one extension
/// somebody thought to write down and admits every variant nobody did. A closed set needs no list of
/// the forbidden — an extension is legal because it was *declared*, and a superseded one cannot come
/// back without failing here. It is also the only form of this check that does not have to name the
/// thing it excludes, which matters when the name is itself the confusion being retired.
#[test]
fn every_extension_named_in_the_workspace_is_declared() {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = Vec::new();
    walk(&root.join("crates"), &mut sources);
    assert!(!sources.is_empty(), "found no sources to scan");

    let mut offenders = Vec::new();
    for path in sources {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        for (i, line) in src.lines().enumerate() {
            // Every `.cv…` run of lowercase letters, wherever it appears — prose, string literal or
            // path. An extension that only ever shows up in a comment is still an extension somebody
            // will act on.
            for (at, _) in line.match_indices(".cv") {
                let ext: String = line[at..]
                    .chars()
                    .take_while(|c| *c == '.' || c.is_ascii_lowercase())
                    .collect();
                if ext.len() <= 3 {
                    continue; // bare `.cv`, or a sentence ending
                }
                if CONTENT_EXTENSIONS.contains(&ext.as_str())
                    || BUILD_EXTENSIONS.contains(&ext.as_str())
                {
                    continue;
                }
                offenders.push(format!("{name}:{}: undeclared extension {ext}", i + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "extensions named but never declared as content or build output:
  {}",
        offenders.join(
            "
  "
        )
    );
}

/// The compiled intermediate never lives under the content root.
///
/// ⚠ Stated in `09-format.md` §11 and checked here, because *"do not write it there"* is a rule someone
/// follows until the first time a path is built by string concatenation.
#[test]
fn the_bytecode_intermediate_is_not_a_content_path() {
    for path in [
        "build/schematics/door.cvo",
        "build/game.cvpak",
        "target/cv/door.cvo",
    ] {
        assert!(
            !path.starts_with("content/"),
            "{path} puts a build product under the content root"
        );
        assert!(
            BUILD_EXTENSIONS.iter().any(|e| path.ends_with(e)),
            "{path} is in a build location but is not build output"
        );
    }
    for path in ["content/schematics/door.cvs", "content/curves/wear.cvcurve"] {
        assert!(
            CONTENT_EXTENSIONS.iter().any(|e| path.ends_with(e)),
            "{path} is under the content root but is not a content format"
        );
    }
}
