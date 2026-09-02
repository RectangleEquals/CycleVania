//! **M11's green condition** — all three formats round-trip, and the documents are the design's own.
//!
//! ⚠ **The fixtures are copied from `09-format.md`, not invented here.** A round-trip test over
//! documents the parser's author wrote proves the parser agrees with itself; over the *specification's*
//! examples it proves the parser agrees with the design. Only the second is worth running.

use cv_cvb::format::{
    check_dials, check_node_blocks, check_ops, format_of, fragment_format, may_paste, Format,
    FormatError,
};
use cv_cvb::parse::parse;
use cv_cvb::write::write;

/// `09-format.md` §6 — a schematic, in full.
const SCHEMATIC: &str = r#"
Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=sch_8f3a2b91
   Tag=Tag'Item.Tool.Tether'

   Begin Component Name="mesh" Type=Kind'/Core/MeshComponent' Id=cmp_01
      Asset=Asset'/Content/Meshes/hookshot.glb'
      Surfaces=(mat_shingle=Kind'/Content/Surfaces/Shingle',mat_stone=Kind'/Content/Surfaces/Stone')
   End Component

   Begin Component Name="rope" Type=Kind'/Content/Components/TetherComponent' Id=cmp_02
      Length=(value=30.0,min=8.0,max=200.0)
   End Component

   Begin Dial Name="length" Kind=Number Id=dial_04
      Type=float
      Default=30.0
      Min=8.0
      Max=200.0
      Doc="how far the rope reaches"
   End Dial

   Begin Dial Name="wear_rate" Kind=Curve Id=dial_05
      Asset=Asset'/Content/Curves/wear.cvcurve'
      Row="rate"
   End Dial

   Begin Dial Name="grade" Kind=Enum Id=dial_06
      Enum=Enum'/Core/ItemClass'
      Default=PROGRESSION
   End Dial

   Begin Graph Name="requires" Role=Hook Id=grf_03
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
   End Graph
End Schematic
"#;

/// `09-format.md` §9.1 — `.cvspine`, in full.
const SPINE: &str = r#"
Begin Spine Version=1 Path=/Content/Spines/Ascent Extends=Kind'/Core/Spine' Id=spn_01
   AppliesTo=Enum'/Core/NodeKind'.REACH
   Strictness=REQUIRED
   Adherence=0.8
   Coverage=CONTIGUOUS

   Begin Slot Name="entrance" Role=Entrance Scope=Area Id=slt_01
      Purpose=Kind'/Content/SlotPurposes/Threshold'
      Shape (MinDegree=2)
   End Slot

   Begin Slot Name="wing_a" Scope=Area Id=slt_03
      Strictness=PREFERRED
      Contents (MustNotContain=(Kind'/Content/Actors/Merchant'))
      Begin Dial Name="room_count" Kind=Adaptive Id=dial_04
         SoftMin=3
         HardMax=5
      End Dial
   End Slot

   Begin Slot Name="capstone" Role=Capstone Scope=Area Id=slt_05
      Shape (MinDegree=3, MaxDegree=4, AdjacentTo=("treasury"))
      Pacing (MinSphere=3)
      Grants=Asset'/Content/Progression/unlocks.cvunlock'#"AscentSeal"
   End Slot

   Begin Group Name="wings" Predecessor="precursor" Successor="capstone" Id=grp_01
      Members=("wing_a","wing_b")
   End Group

   Begin Segment From="precursor" To="capstone" Id=seg_03
      Length=(SoftMin=3,HardMax=9)
      Repeat=(SubSpine=Asset'/Content/Spines/CombatArena.cvspine',SoftMin=3,HardMax=5)

      Begin Fill Name="rubble" Scope=Space Id=fil_01
         Begin Node Id=n_0001 Op=fill.scope_floors Pos=(-320,0)
            Pin (Name=out, Dir=Out, Type=Array'/Core/Floor', To=(n_0002.sites))
         End Node
         Begin Node Id=n_0002 Op=fill.filter_slope Pos=(-160,0)
            Pin (Name=sites,       Dir=In,  Type=Array'/Core/Floor')
            Pin (Name=max_degrees, Dir=In,  Type=float, Value=30.0)
            Pin (Name=out,         Dir=Out, Type=Array'/Core/Floor', To=(n_0005.sites))
         End Node
         Begin Node Id=n_0005 Op=fill.place Pos=(320,0)
            Pin (Name=sites,   Dir=In, Type=Array'/Core/Floor')
            Pin (Name=density, Dir=In, Type=float, Value=1.0)
            Pin (Name=content, Dir=In, Type=TagQuery, Value=Tag'Prop.Debris')
         End Node
      End Fill
   End Segment
End Spine
"#;

/// `09-format.md` §9.2 — `.cvstate`, in full.
const STATE: &str = r#"
Begin StateGraph Version=1 Path=/Content/States/WaterLevel Extends=Kind'/Core/StateGraph' Id=stg_01
   Variable="water_level"
   Scope=Enum'/Core/InstanceScope'.SPACE

   Begin State Name="low" Id=stt_01 Pos=(-160,0)
      Initial=true
      Doc="the plaza drains to the lower walkway"
   End State

   Begin State Name="mid" Id=stt_02 Pos=(0,0)
   End State

   Begin State Name="high" Id=stt_03 Pos=(160,0)
   End State

   Begin Transition From="low" To="mid" Id=trn_01
      Gate=(Form=HoldsRule,unlock=Asset'/Content/Progression/unlocks.cvunlock'#"Song")
      Via=Kind'/Content/Actors/Plaque'
      Cost=(Form=TimeCost,limit=30.0,speed=1.0)
   End Transition

   Begin Transition From="high" To="mid" Id=trn_04
      Gate=(Form=HoldsRule,unlock=Asset'/Content/Progression/unlocks.cvunlock'#"IronBoots")
      Via=Kind'/Content/Actors/Plaque'
   End Transition
End StateGraph
"#;

fn round_trip_is_a_fixed_point(src: &str) {
    let once = write(&parse(src).expect("the design's own example parses"));
    let twice = write(&parse(&once).expect("what we wrote parses back"));
    assert_eq!(once, twice, "parse → write → parse must be a fixed point");
    assert_eq!(
        parse(&once).unwrap(),
        parse(&twice).unwrap(),
        "and the documents must be equal, not merely the bytes"
    );
}

#[test]
fn a_schematic_round_trips() {
    round_trip_is_a_fixed_point(SCHEMATIC);
    let doc = parse(SCHEMATIC).unwrap();
    assert_eq!(format_of(&doc), Ok(Format::Schematic));
    assert_eq!(check_ops(&doc, Format::Schematic), Ok(()));
    assert_eq!(check_node_blocks(&doc, Format::Schematic), Ok(()));
}

#[test]
fn a_spine_round_trips() {
    round_trip_is_a_fixed_point(SPINE);
    let doc = parse(SPINE).unwrap();
    assert_eq!(format_of(&doc), Ok(Format::Spine));
    assert_eq!(check_ops(&doc, Format::Spine), Ok(()));
    assert_eq!(check_node_blocks(&doc, Format::Spine), Ok(()));
}

#[test]
fn a_state_graph_round_trips() {
    round_trip_is_a_fixed_point(STATE);
    let doc = parse(STATE).unwrap();
    assert_eq!(format_of(&doc), Ok(Format::StateGraph));
    assert_eq!(check_ops(&doc, Format::StateGraph), Ok(()));
}

#[test]
fn a_begin_dial_block_round_trips_identically_out_of_a_schematic_and_a_spine() {
    // ⚠ The one block both vocabularies spell the same way, because it is the same concept with a
    // different owner. If the two ever wrote it differently, "shared block" would be a claim rather
    // than a fact.
    let from_schematic = parse(SCHEMATIC)
        .unwrap()
        .blocks("Dial")
        .into_iter()
        .find(|d| d.header_get("Name").unwrap().to_string() == "\"length\"")
        .cloned();

    let shared = "Begin Dial Name=\"length\" Kind=Number Id=dial_04\n   Type=float\n   \
                  Default=30.0\n   Min=8.0\n   Max=200.0\n   Doc=\"how far the rope reaches\"\n\
                  End Dial\n";

    let in_schematic = format!(
        "Begin Schematic Version=1 Path=/Content/x Id=s\n{}End Schematic\n",
        indent(shared)
    );
    let in_spine = format!(
        "Begin Spine Version=1 Path=/Content/y Id=p\n   Begin Slot Name=\"a\" Id=slt\n{}   End Slot\nEnd Spine\n",
        indent(&indent(shared))
    );

    let a = extract_dial(&in_schematic);
    let b = extract_dial(&in_spine);
    assert_eq!(a, b, "the same dial must serialise identically in both");
    assert!(from_schematic.is_some(), "and the design's example has one");
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("   {l}\n")).collect::<String>()
}

/// The canonical bytes of the first `Begin Dial` anywhere in a document.
fn extract_dial(src: &str) -> String {
    fn find(b: &cv_cvb::parse::Block) -> Option<String> {
        if b.kind == "Dial" {
            return Some(write(b));
        }
        b.children().into_iter().find_map(find)
    }
    find(&parse(src).expect("parses")).expect("has a dial")
}

#[test]
fn a_fragment_pastes_into_its_own_format_and_is_refused_by_the_others() {
    let fragment = parse(
        "Begin Fragment Version=1 Format=Schematic Source=/Content/Items/Hookshot Graph=grf_03\n   \
         Begin Node Id=n_0002 Op=array.is_empty Pos=(-80,0)\n      \
         Pin (Name=value, Dir=In, Type=Array<Ref'/Core/Object'>)\n      \
         Pin (Name=out, Dir=Out, Type=bool, To=(n_0003.cond))\n   End Node\nEnd Fragment\n",
    )
    .unwrap();

    assert_eq!(fragment_format(&fragment), Ok(Format::Schematic));
    assert_eq!(may_paste(&fragment, Format::Schematic), Ok(()));
    for refused in [Format::Spine, Format::StateGraph] {
        assert_eq!(
            may_paste(&fragment, refused),
            Err(FormatError::CrossFormatPaste {
                from: Format::Schematic,
                into: refused
            })
        );
    }
}

#[test]
fn a_dial_node_pasted_where_its_dial_is_undeclared_is_flagged_rather_than_rebound() {
    // ⚠ A same-named dial on the destination is a *different* dial. Rebinding would make the fragment
    // mean something different where it landed.
    let fragment = parse(
        "Begin Fragment Version=1 Format=Schematic Source=/Content/Items/Hookshot\n   \
         Begin Node Id=n_0007 Op=/Content/Items/Hookshot.length#dial Pos=(0,0)\n      \
         Pin (Name=out, Dir=Out, Type=float)\n   End Node\nEnd Fragment\n",
    )
    .unwrap();

    assert_eq!(
        check_dials(&fragment, &["/Content/Items/Hookshot.length"]),
        Ok(())
    );

    let err = check_dials(&fragment, &["/Content/Items/Grapple.length"]).unwrap_err();
    assert!(
        matches!(err, FormatError::UnresolvedDial { .. }),
        "a same-named dial on another owner must not satisfy it"
    );
    assert!(err.to_string().contains("never rebound"));
}

#[test]
fn a_fragment_round_trips_like_any_other_document() {
    // ⚠ Any subtree is a valid fragment, because every block is self-describing — so the writer must
    // not need a root type it recognises.
    round_trip_is_a_fixed_point(
        "Begin Fragment Version=1 Format=Spine Source=/Content/Spines/Ascent\n   \
         Begin Node Id=n_0001 Op=fill.scatter Pos=(-80,0)\n      \
         Pin (Name=min_spacing, Dir=In, Type=float, Value=4.0)\n   End Node\nEnd Fragment\n",
    );
}

#[test]
fn the_three_formats_have_three_extensions_and_three_clipboards() {
    let mut seen = std::collections::BTreeSet::new();
    for f in Format::ALL {
        assert!(seen.insert(f.extension()), "{f} shares an extension");
        assert!(seen.insert(f.name()), "{f} shares a name");
    }
    assert_eq!(Format::ALL.len(), 3);
}
