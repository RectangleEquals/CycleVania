//! M10 exit criteria: no accessible state strands the goal.
//!
//! Three things have to hold, and they are genuinely different claims:
//!
//! 1. **Injected traps are caught.** Hand-built softlocks — the ones a real generator produces by
//!    accident — must be detected. A safety analysis that never fires is indistinguishable from one
//!    that does not work.
//! 2. **Generated worlds end up safe.** Run the whole pipeline with one-way commits enabled across
//!    many seeds, repair, and verify nothing strands.
//! 3. **The cost is bounded and measured.** The pass enumerates token sets, so its price has to
//!    be a known quantity rather than a hope.
//!
//! Note that (2) is meaningless without (1): a detector that always returns "safe" would pass the
//! property test trivially. They are tested together for that reason.

use cv_core::{
    Linearity, LinearityResolver, Location, LocationId, MissionEdge, MissionGraph, NodeGraph,
    NodeState, ObjectId, Rule, SoftlockAnalyzer, SoftlockKind, Solver,
};
use cv_determinism::{Aabb, Rng, Vec3};
use std::collections::BTreeMap;

fn cap(i: usize) -> ObjectId {
    ObjectId::derived("token", &format!("cap_{i}"))
}
fn item(i: usize) -> ObjectId {
    ObjectId::derived("item", &format!("item_{i}"))
}

fn world(areas: usize, per_area: usize) -> (NodeGraph, Vec<cv_core::Handle<cv_core::Node>>) {
    let mut g = NodeGraph::new(1.0, 1);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let mut spaces = Vec::new();
    let mut firsts = Vec::new();
    for a in 0..areas {
        let area = g.add_child(reach, format!("area_{a}")).unwrap();
        let rooms: Vec<_> = (0..per_area)
            .map(|s| g.add_child(area, format!("space_{a}_{s}")).unwrap())
            .collect();
        for w in rooms.windows(2) {
            g.connect(w[0], w[1]).unwrap();
        }
        firsts.push(rooms[0]);
        spaces.extend(rooms);
    }
    for a in 1..areas {
        g.connect(spaces[a * per_area - 1], firsts[a]).unwrap();
    }
    for h in g.walk() {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
            .unwrap();
    }
    for h in g.walk() {
        g.advance(h, NodeState::Realized).unwrap();
    }
    (g, spaces)
}

// ---------------------------------------------------------------------------------------------
// 1. Injected traps are caught
// ---------------------------------------------------------------------------------------------

#[test]
fn the_classic_trap_is_detected() {
    // Key before a one-way drop, goal beyond it needing the key. Solvable; also a trap.
    let (_, r) = world(1, 4);
    let mut m = MissionGraph::new(r[0]);
    m.add_edge(MissionEdge::open(r[0], r[1]));
    m.add_edge(MissionEdge::open(r[1], r[2]).one_way());
    m.add_edge(MissionEdge::gated(r[2], r[3], Rule::has(cap(0))));
    m.set_goal(r[3]);
    m.add_location(
        LocationId(0),
        Location {
            scope: r[1],
            slot: 0,
        },
    );

    let placements: BTreeMap<LocationId, ObjectId> =
        [(LocationId(0), item(0))].into_iter().collect();
    let grants: BTreeMap<ObjectId, ObjectId> = [(item(0), cap(0))].into_iter().collect();

    let analysis = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
    assert!(!analysis.is_un_softlockable());
    assert_eq!(analysis.hazards.len(), 1);
    assert!(
        analysis.hazards[0].holding.is_empty(),
        "crossing empty-handed is the trap"
    );
}

#[test]
fn a_trap_needing_partial_progress_is_detected() {
    // Subtler: the player needs cap_0 to reach the drop, and cap_1 (left behind) to finish. Holding
    // exactly cap_0 traps them — an intermediate state, not an empty one.
    let (_, r) = world(1, 5);
    let mut m = MissionGraph::new(r[0]);
    m.add_edge(MissionEdge::open(r[0], r[1]));
    m.add_edge(MissionEdge::gated(r[1], r[2], Rule::has(cap(0)))); // needs cap_0
    m.add_edge(MissionEdge::open(r[2], r[3]).one_way()); // the commit
    m.add_edge(MissionEdge::gated(r[3], r[4], Rule::has(cap(1)))); // needs cap_1
    m.set_goal(r[4]);
    m.add_location(
        LocationId(0),
        Location {
            scope: r[0],
            slot: 0,
        },
    ); // cap_0 early
    m.add_location(
        LocationId(1),
        Location {
            scope: r[1],
            slot: 0,
        },
    ); // cap_1 skippable

    let placements: BTreeMap<LocationId, ObjectId> =
        [(LocationId(0), item(0)), (LocationId(1), item(1))]
            .into_iter()
            .collect();
    let grants: BTreeMap<ObjectId, ObjectId> =
        [(item(0), cap(0)), (item(1), cap(1))].into_iter().collect();

    let analysis = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
    assert!(
        !analysis.is_un_softlockable(),
        "skipping cap_1 must be caught"
    );
    let trap = analysis
        .hazards
        .iter()
        .find(|h| h.holding.contains(&cap(0)) && !h.holding.contains(&cap(1)))
        .expect("the trapping state is 'holds cap_0, skipped cap_1'");
    assert_eq!(trap.kind, SoftlockKind::GoalInaccessible);
}

#[test]
fn a_safe_one_way_is_not_flagged() {
    // A detector that flags every one-way edge is useless. This drop leads onward to the goal with
    // nothing required, so it must pass.
    let (_, r) = world(1, 4);
    let mut m = MissionGraph::new(r[0]);
    m.add_edge(MissionEdge::open(r[0], r[1]));
    m.add_edge(MissionEdge::open(r[1], r[2]).one_way());
    m.add_edge(MissionEdge::open(r[2], r[3]));
    m.set_goal(r[3]);

    let analysis = SoftlockAnalyzer::new(&m, &BTreeMap::new(), &BTreeMap::new()).analyze();
    assert!(analysis.is_un_softlockable());
    assert_eq!(
        analysis.commits_checked, 1,
        "the edge was examined, not skipped"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Generated worlds end up safe
// ---------------------------------------------------------------------------------------------

/// A generated world: its mission graph, what was placed where, and what each item grants.
type GeneratedWorld = (
    MissionGraph,
    BTreeMap<LocationId, ObjectId>,
    BTreeMap<ObjectId, ObjectId>,
);

/// Build a world with gating, cycles and one-way commits, then fill it.
///
/// **Order matters, and it is why this fixture is trap-prone.** One-way commits are added *before*
/// gating, so a gate can land behind a commit — which is precisely the shape that strands a player who
/// dropped early. Gating first would let the fill see every gate when it chose placements, and the
/// resulting worlds would be far safer by construction. A property test over safe-by-luck worlds would
/// prove nothing, so the fixture deliberately builds the dangerous order.
fn generate(
    g: &NodeGraph,
    spaces: &[cv_core::Handle<cv_core::Node>],
    items: usize,
    one_way_fraction: f64,
    linearity: Linearity,
    seed: u64,
) -> Option<GeneratedWorld> {
    let caps: Vec<ObjectId> = (0..items).map(cap).collect();
    let pool: Vec<ObjectId> = (0..items).map(item).collect();

    let resolver = LinearityResolver::new(linearity);
    let mut solver = Solver::new(g, &resolver);
    for i in 0..items {
        solver = solver.with_grant(item(i), cap(i));
    }

    let mut m = MissionGraph::from_scopes(g, spaces[0]);
    for (i, s) in spaces.iter().enumerate() {
        m.add_location(LocationId(i as u32), Location { scope: *s, slot: 0 });
    }
    m.set_goal(*spaces.last().unwrap());

    let rng = Rng::new(seed);
    solver.add_cycles(&mut m, &rng);
    solver.add_one_way_commits(&mut m, one_way_fraction, &rng);
    solver.gate_edges(&mut m, &caps, 0.6, &rng);

    let solution = solver.fill(&m, &pool, &rng).ok()?;
    Some((m, solution.placements, solver.grants().clone()))
}

#[test]
fn generated_worlds_are_made_un_softlockable() {
    // The property, over many seeds: generate with real one-way commits, repair, verify. Also counts
    // how often a trap actually appeared — a run where none did would prove nothing.
    // No cycles: loops are themselves a softlock mitigation (see the test below), so isolating the
    // hazard means removing the escape routes.
    let dangerous = Linearity::new(0.5, 0.0);
    let (g, spaces) = world(3, 4);
    let mut generated = 0;
    let mut had_hazards = 0;

    for seed in 0..60u64 {
        let Some((mut m, placements, grants)) = generate(&g, &spaces, 3, 0.5, dangerous, seed)
        else {
            continue; // an unfillable gating; M09 already covers that path
        };
        generated += 1;

        let before = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
        assert!(
            before.limit.is_none(),
            "seed {seed}: analysis must complete"
        );

        if !before.hazards.is_empty() {
            had_hazards += 1;
            before.repair(&mut m);
            let after = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
            assert!(
                after.is_un_softlockable(),
                "seed {seed}: still {} hazard(s) after repair",
                after.hazards.len()
            );
        }
    }

    assert!(
        generated > 40,
        "only {generated} worlds generated; the sweep is too thin"
    );
    assert!(
        had_hazards > 0,
        "no seed produced a trap — the property test proved nothing about detection"
    );
}

#[test]
fn loops_are_themselves_a_softlock_mitigation() {
    // Found while investigating why the property test initially saw no hazards: with cycles enabled,
    // one-way commits almost never strand anyone, because a loop *is* a return route. That is worth
    // pinning — it means `cycle_density` is not only an aesthetic dial, it materially reduces how
    // often the repair pass has to intervene, and a dev turning it to zero should expect more edges
    // to be made reversible on their behalf.
    let (g, spaces) = world(3, 4);
    let mut hazards_without_loops = 0usize;
    let mut hazards_with_loops = 0usize;

    for seed in 0..40u64 {
        for (linearity, tally) in [
            (Linearity::new(0.5, 0.0), &mut hazards_without_loops),
            (Linearity::new(0.5, 0.8), &mut hazards_with_loops),
        ] {
            if let Some((m, placements, grants)) = generate(&g, &spaces, 3, 0.5, linearity, seed) {
                *tally += SoftlockAnalyzer::new(&m, &placements, &grants)
                    .analyze()
                    .hazards
                    .len();
            }
        }
    }

    assert!(
        hazards_without_loops > 0,
        "the loop-free fixture must produce hazards to compare"
    );
    assert!(
        hazards_with_loops < hazards_without_loops,
        "loops should reduce stranding ({hazards_with_loops} with vs {hazards_without_loops} without)"
    );
}

#[test]
fn repair_preserves_solvability() {
    // Making an edge reversible can only *add* accessibility, so a repaired world must still complete.
    // Worth asserting rather than assuming: a repair that broke the M09 guarantee would be a poor trade.
    let (g, spaces) = world(3, 4);
    for seed in 0..40u64 {
        let Some((mut m, placements, grants)) =
            generate(&g, &spaces, 3, 0.4, Linearity::new(0.5, 0.0), seed)
        else {
            continue;
        };
        let analysis = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
        analysis.repair(&mut m);

        let sweep = m.sweep(&Default::default(), &placements, &grants);
        assert!(
            sweep.accessible(m.goal().unwrap()),
            "seed {seed}: repair broke completability"
        );
        for i in 0..3 {
            assert!(
                sweep.held.contains(&cap(i)),
                "seed {seed}: cap_{i} lost after repair"
            );
        }
    }
}

#[test]
fn a_world_without_commits_needs_no_repair() {
    // Monotone tokens mean collecting cannot strand you. With no one-way edges the pass should
    // find nothing and cost nothing.
    let (g, spaces) = world(3, 4);
    for seed in 0..20u64 {
        let Some((m, placements, grants)) =
            generate(&g, &spaces, 3, 0.0, Linearity::default(), seed)
        else {
            continue;
        };
        let analysis = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
        assert!(analysis.is_un_softlockable());
        assert_eq!(analysis.commits_checked, 0);
        assert_eq!(analysis.states_examined, 0);
    }
}

// ---------------------------------------------------------------------------------------------
// 3. The cost is bounded and measured
// ---------------------------------------------------------------------------------------------

#[test]
fn the_cost_is_measured_and_stays_modest() {
    // ▶ The no-softlock cost model. Monotone pruning is what makes enumerating token sets
    // affordable; this records the actual figure so a regression in the pruning is visible rather
    // than merely slow.
    let (g, spaces) = world(4, 5);
    let mut worst_states = 0usize;
    let mut worst_commits = 0usize;

    for items in [2usize, 4, 6] {
        for seed in 0..20u64 {
            let Some((m, placements, grants)) =
                generate(&g, &spaces, items, 0.4, Linearity::default(), seed)
            else {
                continue;
            };
            let analysis = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
            assert!(analysis.limit.is_none());
            worst_states = worst_states.max(analysis.states_examined);
            worst_commits = worst_commits.max(analysis.commits_checked);
        }
    }

    // With 6 tokens the naive lattice is 64 sets *per commit*; pruning must keep the real
    // figure far below commits × 64.
    let naive = worst_commits * 64;
    assert!(
        worst_states < naive,
        "pruning is not working: {worst_states} states vs a naive {naive}"
    );
    assert!(
        worst_states > 0,
        "the pass must actually have examined something"
    );
}

#[test]
fn an_oversized_world_declines_rather_than_stalling() {
    // A bounded honest failure beats an unbounded wait — and must never read as "safe".
    let (g, spaces) = world(2, 4);
    let Some((m, placements, grants)) = generate(&g, &spaces, 5, 0.4, Linearity::default(), 1)
    else {
        panic!("fixture must generate");
    };
    let analysis = SoftlockAnalyzer::new(&m, &placements, &grants)
        .with_max_tokens(2)
        .analyze();
    assert!(analysis.limit.is_some());
    assert!(!analysis.is_un_softlockable(), "declined is not safe");
}

#[test]
fn analysis_is_deterministic_across_runs() {
    let (g, spaces) = world(3, 4);
    let (m, placements, grants) = generate(&g, &spaces, 3, 0.4, Linearity::default(), 7).unwrap();
    let a = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
    let b = SoftlockAnalyzer::new(&m, &placements, &grants).analyze();
    assert_eq!(a, b);
}
