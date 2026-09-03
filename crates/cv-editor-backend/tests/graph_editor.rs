//! **M18's green condition** — a hook graph is authored, saved, reloaded and compiled; and a fragment
//! pasted from a spine into a schematic is refused with a reason.
//!
//! ⚠ **Authored means *through the palette*.** Every node below is placed by looking one up — there is
//! no text field to type a wrong name into, and that is the structural argument the whole pivot rests
//! on. A test that constructed nodes from string literals would be testing a graph editor this design
//! does not have.

use cv_bindings::DialKind;
use cv_cvb::format::{may_paste, Format, FormatError};
use cv_cvb::parse::parse;
use cv_cvb::write::write;
use cv_editor_backend::connect::{may_connect, Dir, Pin, Refusal};
use cv_editor_backend::palette::{Palette, ProjectDial, Shape, Source};

/// A graph being authored on the canvas.
struct Canvas {
    palette: Palette,
    nodes: Vec<(String, String)>,
    wires: Vec<(String, String, String, String)>,
}

impl Canvas {
    fn new(palette: Palette) -> Self {
        Canvas {
            palette,
            nodes: Vec::new(),
            wires: Vec::new(),
        }
    }

    /// ⚠ **Place a node by looking it up.** A node the palette does not offer cannot be placed at all,
    /// which is what makes a typo not a category of mistake.
    fn place(&mut self, id: &str, op: &str) -> &mut Self {
        assert!(
            self.palette.get(op).is_some(),
            "{op} is not in the palette, so it cannot be placed"
        );
        self.nodes.push((id.into(), op.into()));
        self
    }

    fn wire(&mut self, from: (&str, &str, &str), to: (&str, &str, &str)) -> Result<(), Refusal> {
        may_connect(
            &Pin::new(from.1, Dir::Out, from.2),
            &Pin::new(to.1, Dir::In, to.2),
        )?;
        self.wires
            .push((from.0.into(), from.1.into(), to.0.into(), to.1.into()));
        Ok(())
    }

    /// Save to CVB.
    fn save(&self, hook: &str) -> String {
        let mut out = format!(
            "Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=sch_01\n   \
             Begin Graph Name=\"{hook}\" Role=Hook Id=grf_01\n"
        );
        for (id, op) in &self.nodes {
            out.push_str(&format!("      Begin Node Id={id} Op={op} Pos=(0,0)\n"));
            let outgoing: Vec<&(String, String, String, String)> =
                self.wires.iter().filter(|w| &w.0 == id).collect();
            if outgoing.is_empty() {
                out.push_str("         Pin (Name=out, Dir=Out, Type=Array<Ref'/Core/Object'>)\n");
            }
            for (_, from_pin, to_node, to_pin) in outgoing {
                out.push_str(&format!(
                    "         Pin (Name={from_pin}, Dir=Out, Type=Array<Ref'/Core/Object'>, To=({to_node}.{to_pin}))\n"
                ));
            }
            out.push_str("      End Node\n");
        }
        out.push_str("   End Graph\nEnd Schematic\n");
        out
    }
}

fn palette_with_the_projects_dial() -> Palette {
    let mut p = Palette::generated();
    p.rebuild_project_nodes(&[ProjectDial {
        owner: "/Content/Items/Hookshot".into(),
        name: "length".into(),
        kind: DialKind::Number,
        enum_path: None,
    }]);
    p
}

#[test]
fn a_hook_graph_is_authored_saved_reloaded_and_compiled() {
    let mut canvas = Canvas::new(palette_with_the_projects_dial());

    // Every node comes out of the palette.
    canvas
        .place("n_0001", "/Content/Items/Hookshot.length#dial")
        .place("n_0002", "util.reroute");

    canvas
        .wire(
            ("n_0001", "out", "Array<Ref'/Core/Object'>"),
            ("n_0002", "value", "Array<Ref'/Core/Object'>"),
        )
        .expect("the wire draws");

    let saved = canvas.save("grants");

    // Reloaded.
    let doc = parse(&saved).expect("what the editor saved parses");
    let graph = doc.blocks("Graph")[0];
    assert_eq!(graph.blocks("Node").len(), 2);

    // ⚠ A save is a fixed point, or every save would produce a diff nobody made.
    let once = write(&doc);
    assert_eq!(once, write(&parse(&once).unwrap()));

    // Compiled.
    let compiled = cv_compile::compile(&doc);
    assert!(
        compiled.succeeded(),
        "errors: {:?}",
        compiled.findings().of(cv_compile::Severity::Error)
    );
    assert_eq!(compiled.programs().len(), 1);
}

#[test]
fn a_node_the_palette_does_not_offer_cannot_be_placed() {
    // ⚠ There is no text field to type a wrong name into.
    let palette = Palette::generated();
    assert!(palette
        .get("/Core/Object.definitely_not_a_member#get")
        .is_none());
    assert!(palette.get("core.brnach").is_none());
}

#[test]
fn a_dial_get_node_appears_only_because_the_project_declares_it() {
    // ⚠ Manifest-first is unbroken — there is no core dial to declare there.
    let generated = Palette::generated();
    assert!(generated
        .get("/Content/Items/Hookshot.length#dial")
        .is_none());

    let with_project = palette_with_the_projects_dial();
    let node = with_project
        .get("/Content/Items/Hookshot.length#dial")
        .expect("the project's dial is offered");
    assert_eq!(node.source, Source::Project);
    assert_eq!(node.shape, Shape::Pure, "a dial read sequences nothing");
    assert_eq!(node.out_type.as_deref(), Some("float"));
}

#[test]
fn deleting_the_dial_removes_its_node_on_the_next_save() {
    // ⚠ The project-sourced half is the one that can go stale mid-session, which is why it rebuilds on
    // save rather than at startup.
    let mut palette = palette_with_the_projects_dial();
    assert!(palette.get("/Content/Items/Hookshot.length#dial").is_some());

    palette.rebuild_project_nodes(&[]);
    assert!(
        palette.get("/Content/Items/Hookshot.length#dial").is_none(),
        "a deleted dial stops being offerable"
    );
    assert!(
        !palette.from(Source::Manifest).is_empty(),
        "and the generated half is untouched"
    );
}

#[test]
fn a_kind_pin_will_not_wire_to_a_ref_pin_so_the_mistake_never_reaches_a_document() {
    // ⚠ Tier 1 is *impossible*, not *error*: the wire does not draw.
    let mut canvas = Canvas::new(Palette::generated());
    let refusal = canvas
        .wire(
            ("n_0001", "out", "Kind<Item>"),
            ("n_0002", "value", "Ref<Item>"),
        )
        .unwrap_err();
    assert!(matches!(refusal, Refusal::KindVersusRef { .. }));
    assert!(
        canvas.wires.is_empty(),
        "a refused wire leaves nothing behind"
    );
}

#[test]
fn execution_flow_will_not_wire_into_a_data_pin() {
    let mut canvas = Canvas::new(Palette::generated());
    assert_eq!(
        canvas.wire(("a", "out", "exec"), ("b", "cond", "bool")),
        Err(Refusal::ExecMismatch)
    );
}

#[test]
fn a_spine_fragment_pasted_into_a_schematic_is_refused_with_a_reason() {
    // ⚠ The second half of the green condition. Near-identical syntax, different meaning.
    let fragment = parse(
        "Begin Fragment Version=1 Format=Spine Source=/Content/Spines/Ascent\n   \
         Begin Node Id=n_0001 Op=fill.scatter Pos=(0,0)\n      \
         Pin (Name=min_spacing, Dir=In, Type=float, Value=4.0)\n   End Node\nEnd Fragment\n",
    )
    .unwrap();

    let refused = may_paste(&fragment, Format::Schematic).unwrap_err();
    assert_eq!(
        refused,
        FormatError::CrossFormatPaste {
            from: Format::Spine,
            into: Format::Schematic
        }
    );
    let text = refused.to_string();
    assert!(text.contains("does not paste into"));
    assert!(
        text.contains("vocabularies differ"),
        "the refusal carries the reason: {text}"
    );

    // And it pastes into its own format.
    assert_eq!(may_paste(&fragment, Format::Spine), Ok(()));
}

#[test]
fn a_schematic_fragment_is_refused_by_a_spine_too() {
    // ⚠ The rule is symmetric — "the reverse is equally false".
    let fragment = parse(
        "Begin Fragment Version=1 Format=Schematic Source=/Content/Items/Hookshot\n   \
         Begin Node Id=n_0001 Op=array.make Pos=(0,0)\n   End Node\nEnd Fragment\n",
    )
    .unwrap();
    assert!(may_paste(&fragment, Format::Spine).is_err());
    assert_eq!(may_paste(&fragment, Format::Schematic), Ok(()));
}

#[test]
fn every_palette_node_has_a_shape_that_says_whether_it_sequences() {
    // ⚠ The shape follows from what the member is, so a reader never has to guess whether a node has
    // exec pins.
    let palette = palette_with_the_projects_dial();
    for node in palette.nodes() {
        match node.shape {
            Shape::Pure | Shape::Form | Shape::Literal => {
                assert!(
                    !node.shape.has_exec_in() && !node.shape.has_exec_out(),
                    "{}",
                    node.op
                );
            }
            Shape::Call => assert!(node.shape.has_exec_in() && node.shape.has_exec_out()),
            Shape::Event => {
                assert!(!node.shape.has_exec_in(), "an event is not called");
                assert!(node.shape.has_exec_out());
            }
        }
    }
}

#[test]
fn every_node_the_palette_offers_is_a_node_the_compiler_accepts() {
    // ⚠ **The guard for the bug this test found.** The palette offered `util.reroute` and the
    // compiler had never heard of it, so an editor could place a node that would not build — two halves
    // of one toolchain disagreeing about what a node *is*. Neither side's own tests could catch that,
    // because each was self-consistent.
    let palette = palette_with_the_projects_dial();
    let mut unbuildable = Vec::new();

    for node in palette.nodes() {
        // A dial read is namespaced by ownership rather than by palette, and the compiler exempts it.
        if node.op.ends_with("#dial") {
            continue;
        }
        let schematic = format!(
            "Begin Schematic Version=1 Path=/Content/X Extends=Kind'/Core/Item' Id=s\n   \
             Begin Graph Name=\"grants\" Role=Hook Id=grf\n      \
             Begin Node Id=n_0001 Op={} Pos=(0,0)\n      End Node\n   End Graph\nEnd Schematic\n",
            node.op
        );
        let Ok(doc) = parse(&schematic) else {
            unbuildable.push(format!("{} — does not parse", node.op));
            continue;
        };
        let compiled = cv_compile::compile(&doc);
        for finding in compiled.findings().of(cv_compile::Severity::Error) {
            if finding.message.contains("no op named") {
                unbuildable.push(format!("{} — {}", node.op, finding.message));
            }
        }
    }

    assert!(
        unbuildable.is_empty(),
        "the palette offers {} node(s) the compiler refuses:\n  {}",
        unbuildable.len(),
        unbuildable.join("\n  ")
    );
}

#[test]
fn every_utility_the_palette_offers_is_an_op_the_vm_can_run() {
    // ⚠ The other half of the same seam: the compiler accepting a name is not the VM knowing what to
    // do with it.
    for u in cv_editor_backend::palette::Utility::ALL {
        let op = cv_vm::ops::Op::from_name(u.op())
            .unwrap_or_else(|| panic!("{} is not in the instruction set", u.op()));
        assert!(
            op.is_pure(),
            "{op} sequences, but the palette draws it pure"
        );
    }
}
