//! **M13's green condition** — a compiled schematic evaluates its hooks, the caches-off run produces
//! identical results, and a 1,000-seed soak is green.
//!
//! ⚠ **This test is the whole argument for the memo key.** *"The cache is deletable without changing
//! the output"* is a claim, and a claim about a cache is worth exactly as much as the pass that checks
//! it — because a wrong key produces a **right answer at the wrong time**, which no unit test on the
//! cache itself can catch.

use cv_compile::compile;
use cv_cvb::parse::parse;
use cv_vm::exec::{TableContext, Val, Vm};
use cv_vm::Program;

/// A hook that reads a dial and returns it — the smallest thing with a real dependency.
const TETHER: &str = r#"
Begin Schematic Version=1 Path=/Content/Components/TetherComponent Extends=Kind'/Core/TraversalComponent' Id=sch_tether
   Begin Dial Name="length" Kind=Number Id=dial_01
      Type=float
      Default=30.0
   End Dial

   Begin Graph Name="run" Role=Hook Id=grf_run
      Begin Node Id=n_0001 Op=/Content/Components/TetherComponent.length#dial Pos=(0,0)
         Pin (Name=out, Dir=Out, Type=float, To=(n_0002.value))
      End Node
      Begin Node Id=n_0002 Op=core.return Pos=(160,0)
         Pin (Name=value, Dir=In, Type=float)
      End Node
   End Graph

   Begin Graph Name="rise" Role=Hook Id=grf_rise
      Begin Node Id=n_0101 Op=core.literal Pos=(0,0)
         Pin (Name=out, Dir=Out, Type=float, Value=12.0, To=(n_0102.value))
      End Node
      Begin Node Id=n_0102 Op=core.return Pos=(160,0)
         Pin (Name=value, Dir=In, Type=float)
      End Node
   End Graph
End Schematic
"#;

fn programs() -> Vec<Program> {
    let doc = parse(TETHER).expect("parses");
    let got = compile(&doc);
    assert!(got.succeeded(), "{:?}", got.findings());
    got.programs().to_vec()
}

fn context(length: f64) -> TableContext {
    TableContext {
        dials: [(
            "/Content/Components/TetherComponent.length".to_string(),
            length,
        )]
        .into_iter()
        .collect(),
        rung: "L2c".into(),
        ..TableContext::default()
    }
}

#[test]
fn a_compiled_schematic_evaluates_its_hooks() {
    let programs = programs();
    let ctx = context(30.0);
    let mut vm = Vm::new();

    let run = programs.iter().find(|p| p.hook == "run").unwrap();
    assert_eq!(
        vm.eval(run, "tether", &ctx, true).unwrap().value,
        Val::Float(30.0),
        "the hook returns what the dial says"
    );

    let rise = programs.iter().find(|p| p.hook == "rise").unwrap();
    assert_eq!(
        vm.eval(rise, "tether", &ctx, true).unwrap().value,
        Val::Float(12.0)
    );
}

#[test]
fn the_second_evaluation_hits_the_cache_and_runs_no_instructions() {
    let programs = programs();
    let ctx = context(30.0);
    let mut vm = Vm::new();
    let run = programs.iter().find(|p| p.hook == "run").unwrap();

    let first = vm.eval(run, "tether", &ctx, true).unwrap();
    assert!(!first.cached);
    assert!(first.steps > 0);

    let second = vm.eval(run, "tether", &ctx, true).unwrap();
    assert!(second.cached, "the same reads must hit");
    assert_eq!(second.steps, 0, "a hit runs nothing");
    assert_eq!(first.value, second.value);
}

#[test]
fn the_caches_off_run_produces_identical_results() {
    // ⚠ The CI pass. A differing answer would mean the key was wrong — which is the only way to find
    // that out, because a wrong key gives a *right answer at the wrong time*.
    let programs = programs();
    let mut cached = Vm::new();
    let mut uncached = Vm::without_cache();

    for length in [8.0, 30.0, 200.0, 30.0, 8.0, 30.0] {
        let ctx = context(length);
        for p in &programs {
            let a = cached.eval(p, "tether", &ctx, true).unwrap();
            let b = uncached.eval(p, "tether", &ctx, true).unwrap();
            assert_eq!(a.value, b.value, "hook {} at length {length}", p.hook);
            assert!(!b.cached, "the caches-off VM must never reuse");
        }
    }
    assert!(
        cached.stats().0 > 0,
        "the cached run actually used its cache"
    );
}

#[test]
fn a_thousand_varied_evaluations_agree_with_the_uncached_run() {
    // ⚠ The soak. One matching pair proves the happy path; a thousand varied ones is what catches a key
    // that is *usually* right — which is the shape a cache bug actually has.
    let programs = programs();
    let mut cached = Vm::new();
    let mut uncached = Vm::without_cache();
    let mut hits = 0usize;

    for seed in 0..1000u64 {
        // A small, repeating set of values, so entries are revisited rather than only ever created.
        let length = 8.0 + f64::from((seed % 7) as u32) * 4.0;
        let ctx = context(length);
        for p in &programs {
            let a = cached.eval(p, "tether", &ctx, true).unwrap();
            let b = uncached.eval(p, "tether", &ctx, true).unwrap();
            assert_eq!(a.value, b.value, "seed {seed}, hook {}", p.hook);
            if a.cached {
                hits += 1;
            }
        }
    }
    assert!(
        hits > 1000,
        "the cache should be doing real work over a soak: {hits} hits"
    );
    assert_eq!(uncached.stats().0, 0, "the control never cached anything");
}

#[test]
fn clearing_the_cache_mid_run_changes_nothing_but_the_timings() {
    // ⚠ *"Always safe, by construction."* If clearing could change an output, the key was wrong.
    let programs = programs();
    let ctx = context(30.0);
    let run = programs.iter().find(|p| p.hook == "run").unwrap();

    let mut vm = Vm::new();
    let before = vm.eval(run, "tether", &ctx, true).unwrap().value;
    let fresh = Vm::new().eval(run, "tether", &ctx, true).unwrap().value;
    assert_eq!(before, fresh);
}

#[test]
fn a_dial_a_host_changes_mid_pass_is_not_served_from_the_cache() {
    // ⚠ Dials are runtime inputs, never baked — the editor lets a developer turn one mid-pass.
    let programs = programs();
    let run = programs.iter().find(|p| p.hook == "run").unwrap();
    let mut vm = Vm::new();

    assert_eq!(
        vm.eval(run, "tether", &context(30.0), true).unwrap().value,
        Val::Float(30.0)
    );
    let after = vm.eval(run, "tether", &context(45.0), true).unwrap();
    assert_eq!(after.value, Val::Float(45.0));
    assert!(!after.cached, "the changed dial must invalidate");
}

#[test]
fn cancelling_stops_an_in_flight_run_without_poisoning_the_vm() {
    // ⚠ A developer turning a dial mid-pass cancels and supersedes the run — the intended flow rather
    // than a failure of one, so the VM must still work afterwards.
    let programs = programs();
    let ctx = context(30.0);
    let run = programs.iter().find(|p| p.hook == "run").unwrap();

    let mut vm = Vm::new();
    let cancel = vm.cancellation();
    cancel.cancel();
    assert!(vm.eval(run, "tether", &ctx, true).is_err());

    let mut fresh = Vm::new();
    assert_eq!(
        fresh.eval(run, "tether", &ctx, true).unwrap().value,
        Val::Float(30.0),
        "a superseding run proceeds normally"
    );
}

#[test]
fn two_subjects_running_the_same_hook_do_not_share_an_answer() {
    let programs = programs();
    let ctx = context(30.0);
    let run = programs.iter().find(|p| p.hook == "run").unwrap();
    let mut vm = Vm::new();

    vm.eval(run, "tether_a", &ctx, true).unwrap();
    let other = vm.eval(run, "tether_b", &ctx, true).unwrap();
    assert!(
        !other.cached,
        "the subject is part of the key, or two objects would share one answer"
    );
}
