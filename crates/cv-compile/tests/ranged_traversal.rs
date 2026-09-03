//! **M12's green condition** — the `ranged-traversal` schematics compile, and a deliberately-broken
//! graph produces a readable error naming the node and the pin.
//!
//! ⚠ **The second half is the one that decides whether the compiler is usable.** Every compiler
//! compiles correct input; what separates a good one is what it says about incorrect input, and a
//! visual language raises the bar rather than lowering it — a developer who cannot find the node is
//! worse off than one reading a stack trace, because there is no line number to fall back on.

use cv_compile::{compile, Compiled, Op, Severity, Ty};
use cv_cvb::parse::parse;

/// The Hookshot's `requires` hook, from `14-scenarios/ranged-traversal.md` §1.4.
///
/// *"For that traversal to exist, something taking my line must be within range and in sight."*
const HOOKSHOT: &str = r#"
Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=sch_hookshot
   Tag=Tag'Item.Tool.Tether'

   Begin Component Name="mesh" Type=Kind'/Core/MeshComponent' Id=cmp_01
      Asset=Asset'/Content/Meshes/hookshot.glb'
   End Component

   Begin Dial Name="length" Kind=Number Id=dial_01
      Type=float
      Default=30.0
      Min=8.0
      Max=200.0
      Doc="how far the rope reaches"
   End Dial

   Begin Graph Name="requires" Role=Hook Id=grf_requires
      Begin Node Id=n_0001 Op=core.instances_of Pos=(-320,0)
         Pin (Name=kind,  Dir=In,  Type=Kind'/Core/Component', Value=Kind'/Content/Components/LatchTargetComponent')
         Pin (Name=scope, Dir=In,  Type=Enum'/Core/InstanceScope', Value=AREA)
         Pin (Name=out,   Dir=Out, Type=Array<Ref'/Content/Components/LatchTargetComponent'>, To=(n_0002.value))
      End Node

      Begin Node Id=n_0002 Op=array.is_empty Pos=(-80,0)
         Pin (Name=value, Dir=In,  Type=Array<Ref'/Core/Object'>)
         Pin (Name=out,   Dir=Out, Type=bool, To=(n_0003.cond))
      End Node

      Begin Node Id=n_0003 Op=core.branch Pos=(120,0)
         Pin (Name=cond,  Dir=In,  Type=bool)
         Pin (Name=true,  Dir=Out, Type=exec, To=(n_0004.in))
         Pin (Name=false, Dir=Out, Type=exec, To=(n_0005.in))
      End Node

      Begin Node Id=n_0004 Op=array.make Pos=(360,-80)
         Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/PlacementNeed'>, To=(n_0006.value))
      End Node

      Begin Node Id=n_0005 Op=array.make Pos=(360,80)
         Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/PlacementNeed'>, To=(n_0006.value))
      End Node

      Begin Node Id=n_0006 Op=core.return Pos=(600,0)
         Pin (Name=value, Dir=In, Type=Array<Ref'/Core/PlacementNeed'>)
      End Node
   End Graph

   Begin Graph Name="grants" Role=Hook Id=grf_grants
      Begin Node Id=n_0101 Op=array.make Pos=(0,0)
         Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Unlock'>, To=(n_0102.value))
      End Node
      Begin Node Id=n_0102 Op=core.return Pos=(160,0)
         Pin (Name=value, Dir=In, Type=Array<Ref'/Core/Unlock'>)
      End Node
   End Graph
End Schematic
"#;

/// The traversal component from §1.2, whose hooks are the five a `TraversalComponent` declares.
const TETHER: &str = r#"
Begin Schematic Version=1 Path=/Content/Components/TetherComponent Extends=Kind'/Core/TraversalComponent' Id=sch_tether
   Begin Dial Name="length" Kind=Number Id=dial_02
      Type=float
      Default=30.0
   End Dial

   Begin Graph Name="run" Role=Hook Id=grf_run
      Begin Node Id=n_0201 Op=/Content/Components/TetherComponent.length#dial Pos=(0,0)
         Pin (Name=out, Dir=Out, Type=float, To=(n_0202.value))
      End Node
      Begin Node Id=n_0202 Op=core.return Pos=(160,0)
         Pin (Name=value, Dir=In, Type=float)
      End Node
   End Graph

   Begin Graph Name="rise" Role=Hook Id=grf_rise
      Begin Node Id=n_0301 Op=core.literal Pos=(0,0)
         Pin (Name=out, Dir=Out, Type=float, Value=12.0, To=(n_0302.value))
      End Node
      Begin Node Id=n_0302 Op=core.return Pos=(160,0)
         Pin (Name=value, Dir=In, Type=float)
      End Node
   End Graph
End Schematic
"#;

fn compiled(src: &str) -> Compiled {
    compile(&parse(src).expect("the scenario's schematic parses"))
}

#[test]
fn the_hookshot_compiles() {
    let got = compiled(HOOKSHOT);
    assert!(
        got.succeeded(),
        "errors: {:?}",
        got.findings().of(Severity::Error)
    );
    let hooks: Vec<&str> = got.programs().iter().map(|p| p.hook.as_str()).collect();
    assert_eq!(hooks, vec!["grants", "requires"]);
}

#[test]
fn the_tether_component_compiles() {
    let got = compiled(TETHER);
    assert!(
        got.succeeded(),
        "errors: {:?}",
        got.findings().of(Severity::Error)
    );
    assert_eq!(got.programs().len(), 2);
}

#[test]
fn the_lowered_program_is_in_dependency_order_and_typed() {
    let got = compiled(HOOKSHOT);
    let requires = got
        .programs()
        .iter()
        .find(|p| p.hook == "requires")
        .expect("the hook the scenario exists for");

    let order: Vec<&str> = requires.instrs.iter().map(|i| i.source.as_str()).collect();
    let at = |id: &str| order.iter().position(|s| *s == id).expect(id);
    assert!(at("n_0001") < at("n_0002"), "{order:?}");
    assert!(at("n_0002") < at("n_0003"), "{order:?}");
    assert!(at("n_0004") < at("n_0006") && at("n_0005") < at("n_0006"));

    let instances_of = &requires.instrs[at("n_0001")];
    assert_eq!(
        instances_of.ty,
        Ty::Array(Box::new(Ty::Ref(
            "/Content/Components/LatchTargetComponent".into()
        ))),
        "the pin's type survives into the instruction"
    );
    assert_eq!(requires.instrs[at("n_0002")].ty, Ty::Bool);
}

#[test]
fn compiling_twice_produces_the_same_program() {
    // ⚠ The order is a property of the graph, not of the run — ties break by content-derived id.
    assert_eq!(compiled(HOOKSHOT).programs(), compiled(HOOKSHOT).programs());
}

#[test]
fn a_dial_read_survives_optimization_because_it_is_a_runtime_input() {
    // ⚠ Nothing in `run` reads the dial's result except the return, but even unread it must not be
    // folded to its authored default — a host sets dials before generating.
    let got = compiled(TETHER);
    let run = got.programs().iter().find(|p| p.hook == "run").unwrap();
    assert!(
        run.instrs.iter().any(|i| i.op == Op::DialRead),
        "the dial read was optimized away: {:?}",
        run.instrs
    );
}

#[test]
fn a_broken_override_names_the_node_and_suggests_the_hook() {
    let broken = HOOKSHOT.replace("Name=\"requires\"", "Name=\"requries\"");
    let got = compiled(&broken);
    assert!(!got.succeeded());

    let errors = got.findings().of(Severity::Error);
    let e = errors[0];
    assert_eq!(e.node, "grf_requires", "the finding names the graph block");
    assert!(e.message.contains("no hook named `requries`"), "{e}");
    assert_eq!(e.hint.as_deref(), Some("did you mean `requires`?"));

    let rendered = e.to_string();
    assert!(
        rendered.starts_with("error: node grf_requires"),
        "{rendered}"
    );
}

#[test]
fn a_broken_op_names_the_node_and_the_nearest_op() {
    let broken = HOOKSHOT.replace("Op=array.is_empty", "Op=array.is_emty");
    let got = compiled(&broken);
    assert!(!got.succeeded());

    let e = got
        .findings()
        .of(Severity::Error)
        .into_iter()
        .find(|f| f.node == "n_0002")
        .expect("the error names the node the developer drew");
    assert!(e.message.contains("no op named `array.is_emty`"));
    assert_eq!(e.hint.as_deref(), Some("did you mean `array.is_empty`?"));
}

#[test]
fn a_broken_link_names_both_ends_and_emits_nothing() {
    let broken = HOOKSHOT.replace("To=(n_0003.cond)", "To=(n_0099.cond)");
    let got = compiled(&broken);
    assert!(!got.succeeded());
    assert!(got.programs().is_empty(), "a failed compile emits nothing");

    let text = got
        .findings()
        .of(Severity::Error)
        .iter()
        .map(|f| f.message.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("n_0002") && text.contains("n_0099"), "{text}");
}

#[test]
fn an_unbounded_loop_names_the_pin_it_is_missing() {
    let broken = HOOKSHOT.replace(
        "Begin Node Id=n_0004 Op=array.make Pos=(360,-80)",
        "Begin Node Id=n_0004 Op=core.for Pos=(360,-80)",
    );
    let got = compiled(&broken);
    assert!(!got.succeeded());
    let e = got
        .findings()
        .of(Severity::Error)
        .into_iter()
        .find(|f| f.node == "n_0004")
        .expect("named the node");
    assert_eq!(e.pin.as_deref(), Some("count"));
    assert!(e.hint.as_deref().unwrap().contains("terminate"));
}

#[test]
fn every_error_carries_something_a_developer_can_click() {
    // ⚠ There is no line number to fall back on in a visual language — the node id is the location.
    for broken in [
        HOOKSHOT.replace("Name=\"requires\"", "Name=\"requries\""),
        HOOKSHOT.replace("Op=array.is_empty", "Op=array.is_emty"),
        HOOKSHOT.replace("To=(n_0003.cond)", "To=(n_0099.cond)"),
    ] {
        let got = compiled(&broken);
        for e in got.findings().of(Severity::Error) {
            assert!(!e.node.is_empty(), "an error with nowhere to go: {e}");
        }
    }
}
