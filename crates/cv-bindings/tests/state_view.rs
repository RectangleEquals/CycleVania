//! **What the State view draws** — read from a real `.cvstate` document.
//!
//! ⚠ **The fixture is the design's own example**, `08-graph-resources.md` §9: a water level whose `high`
//! needs Iron Boots to leave. If the reader cannot handle the document the design draws, it cannot
//! handle the one a developer writes.

use cv_bindings::stategraph::check;

/// `08-graph-resources.md` §9 — the water level, with authored positions.
const WATER: &str = r#"Begin StateGraph Version=1 Path=/Content/States/WaterLevel Id=stg_01
   Variable="water_level"

   Begin State Name="low" Id=stt_01 Pos=(-160,0)
      Initial=true
      Doc="the plaza drains to the lower walkway"
   End State

   Begin State Name="mid" Id=stt_02 Pos=(0,0)
   End State

   Begin State Name="high" Id=stt_03 Pos=(160,0)
   End State

   Begin Transition From="low" To="mid" Id=trn_01
      Via=Kind'/Content/Actors/Plaque'
   End Transition

   Begin Transition From="mid" To="low" Id=trn_02
      Via=Kind'/Content/Actors/Plaque'
   End Transition

   Begin Transition From="mid" To="high" Id=trn_03
      Via=Kind'/Content/Actors/Plaque'
   End Transition

   Begin Transition From="high" To="mid" Id=trn_04
      Gate=(Form=HoldsRule,unlock=Asset'/Content/Progression/unlocks.cvunlock'#"IronBoots")
      Via=Kind'/Content/Actors/Plaque'
   End Transition
End StateGraph
"#;

#[test]
fn it_reads_the_documents_own_positions() {
    // ⚠ **Drawing needs no layout algorithm**, because the author placed the boxes. That is why the
    // State view can be drawn while panel arrangement is still waiting on mockups.
    let json = check(WATER).expect("the design's own example reads");
    assert!(json.contains("\"name\":\"low\""));
    assert!(json.contains("\"x\":-160"), "{json}");
    assert!(json.contains("\"x\":160"), "{json}");
    assert!(json.contains("\"initial\":true"));
}

#[test]
fn it_finds_the_gated_way_back_the_design_draws() {
    // ⚠ *"`high` is accessible but `low` is not accessible FROM `high` without [Iron Boots]"*.
    let json = check(WATER).expect("reads");
    assert!(json.contains("\"kind\":\"exit-gated\""), "{json}");
    assert!(json.contains("IronBoots"), "{json}");
    // ⚠ A gated exit warns without blocking — gating a way back is a legitimate design, and a check
    // that blocked on it is a check somebody turns off.
    assert!(json.contains("\"satisfiesP15\":true"), "{json}");
    assert!(json.contains("\"blocks\":false"), "{json}");
}

#[test]
fn it_fires_on_a_deliberately_broken_variant() {
    // ⚠ **M19's green condition.** Remove the way back and the drawing says so.
    let broken = WATER
        .replace(
            "   Begin Transition From=\"mid\" To=\"low\" Id=trn_02\n      Via=Kind'/Content/Actors/Plaque'\n   End Transition\n\n",
            "",
        )
        .replace(
            "   Begin Transition From=\"high\" To=\"mid\" Id=trn_04\n      Gate=(Form=HoldsRule,unlock=Asset'/Content/Progression/unlocks.cvunlock'#\"IronBoots\")\n      Via=Kind'/Content/Actors/Plaque'\n   End Transition\n",
            "",
        );
    let json = check(&broken).expect("reads");
    assert!(json.contains("\"kind\":\"dead-end\""), "{json}");
    assert!(json.contains("\"satisfiesP15\":false"), "{json}");
    assert!(json.contains("\"blocks\":true"), "{json}");
}

#[test]
fn a_finding_names_the_state_so_the_view_can_draw_it_on_that_box() {
    // ⚠ Telling a developer "nobody can get here" when the truth is "nobody can leave" sends them to
    // the wrong end of the graph — so the four faults are named apart and each carries its state.
    let json = check(WATER).expect("reads");
    assert!(json.contains("\"state\":\"high\""), "{json}");
}

#[test]
fn a_malformed_document_says_so_rather_than_drawing_nothing() {
    let err = check("Begin Unclosed\n").expect_err("a broken document is refused");
    assert!(err.to_string().contains("did not parse"), "{err}");
}
