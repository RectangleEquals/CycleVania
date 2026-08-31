//! M09 exit criteria: mission graphs are **solvable by construction**, cycles appear where the dials
//! ask, and the whole thing is deterministic.
//!
//! The first of those is the one worth stressing. "Solvable by construction" is a strong claim, and a
//! handful of examples does not test it — so the core property here runs across many seeds, several
//! world shapes, and the full range of both linearity dials, asserting every single result is
//! completable. If assumed fill can produce a circular dependency, this is where it shows.
//!
//! The dial tests are the other half: a generator that only makes MP1-shaped worlds is as useless to a
//! Portal-style dev as a linear one is to a metroidvania dev, so "the dials actually change the world"
//! is a requirement, not a nicety.

use cv_core::{
    Location, LocationId, MissionGraph, NodeGraph, NodeState, ObjectId, Rule, SolveError, Solver,
};
use cv_determinism::{Aabb, Rng, Vec3};

fn cap(i: usize) -> ObjectId {
    ObjectId::derived("unlock", &format!("cap_{i}"))
}

fn item(i: usize) -> ObjectId {
    ObjectId::derived("item", &format!("item_{i}"))
}

/// A world of `areas` Areas, each holding `per_area` Spaces in a chain, with the Areas linked.
///
/// Wider than a bare chain so the topology has somewhere to grow loops.
fn build_world(areas: usize, per_area: usize) -> (NodeGraph, Vec<cv_core::Handle<cv_core::Node>>) {
    let mut g = NodeGraph::new(1.0, 1);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let mut all_spaces = Vec::new();
    let mut area_firsts = Vec::new();

    for a in 0..areas {
        let area = g.add_child(reach, format!("area_{a}")).unwrap();
        let spaces: Vec<_> = (0..per_area)
            .map(|s| g.add_child(area, format!("space_{a}_{s}")).unwrap())
            .collect();
        for w in spaces.windows(2) {
            g.connect(w[0], w[1]).unwrap();
        }
        area_firsts.push(spaces[0]);
        all_spaces.extend(spaces);
    }
    // Link consecutive Areas end-to-start.
    for a in 1..areas {
        let prev_last = all_spaces[a * per_area - 1];
        g.connect(prev_last, area_firsts[a]).unwrap();
    }

    for h in g.walk() {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
            .unwrap();
    }
    for h in g.walk() {
        g.advance(h, NodeState::Realized).unwrap();
    }
    (g, all_spaces)
}

fn mission_for(g: &NodeGraph, spaces: &[cv_core::Handle<cv_core::Node>]) -> MissionGraph {
    let mut m = MissionGraph::from_scopes(g, spaces[0]);
    for (i, s) in spaces.iter().enumerate() {
        m.add_location(LocationId(i as u32), Location { scope: *s, slot: 0 });
    }
    m
}

/// Build, gate, cycle and solve a world. Returns the mission and solution.
fn generate(
    g: &NodeGraph,
    spaces: &[cv_core::Handle<cv_core::Node>],
    cycle_density: f64,
    item_count: usize,
    gate_fraction: f64,
    seed: u64,
) -> Result<(MissionGraph, cv_core::Solution), SolveError> {
    let caps: Vec<ObjectId> = (0..item_count).map(cap).collect();
    let items: Vec<ObjectId> = (0..item_count).map(item).collect();

    let mut solver = Solver::new(g).with_cycle_density(cycle_density);
    for i in 0..item_count {
        solver = solver.with_grant(item(i), cap(i));
    }

    let mut mission = mission_for(g, spaces);
    let rng = Rng::new(seed);
    solver.add_cycles(&mut mission, &rng);
    solver.gate_edges(&mut mission, &caps, gate_fraction, &rng);
    let solution = solver.fill(&mission, &items, &rng)?;
    Ok((mission, solution))
}

// ---------------------------------------------------------------------------------------------
// The guarantee
// ---------------------------------------------------------------------------------------------

#[test]
fn every_generated_world_is_completable() {
    // The property the milestone exists to establish, swept across shapes, densities and seeds.
    let shapes = [(2, 4), (3, 3), (4, 5), (1, 10)];
    let densities = [0.0, 0.35, 1.0];

    let mut worlds = 0;
    let mut retried = 0;
    for (areas, per_area) in shapes {
        let (g, spaces) = build_world(areas, per_area);
        for density in densities {
            for seed in 0..25u64 {
                let (_, solution) =
                    generate(&g, &spaces, density, 3, 0.6, seed).unwrap_or_else(|e| {
                        panic!("{areas}x{per_area} density {density} seed {seed}: {e}")
                    });

                // Every unlock is obtainable...
                for i in 0..3 {
                    assert!(
                        solution.accessibility.held.contains(&cap(i)),
                        "{areas}x{per_area} seed {seed}: cap_{i} is unobtainable"
                    );
                }
                // ...and the far end of the world is reached.
                assert!(
                    solution.accessibility.accessible(*spaces.last().unwrap()),
                    "{areas}x{per_area} seed {seed}: the world does not complete"
                );
                worlds += 1;
                if solution.attempts > 1 {
                    retried += 1;
                }
            }
        }
    }
    // 4 shapes x 3 densities x 25 seeds. Was 500 when the sweep ran 5 `Linearity` presets,
    // two of which varied a dial the design refuses.
    assert_eq!(worlds, 300);
    // Retries are legitimate but should be the exception; a high rate means gating is too tight.
    assert!(
        retried * 4 < worlds,
        "{retried}/{worlds} worlds needed a retry — assumed fill is struggling"
    );
}

#[test]
fn a_key_is_never_placed_behind_the_lock_it_opens() {
    // Stated directly rather than inferred from completability: for every placement, the room holding
    // the key must be accessible *without* that key.
    let (g, spaces) = build_world(3, 4);

    for seed in 0..40u64 {
        let (mission, solution) = generate(&g, &spaces, 0.5, 3, 0.7, seed).unwrap();
        for (loc, placed) in &solution.placements {
            let Some(granted) = [0, 1, 2]
                .iter()
                .find(|i| item(**i) == *placed)
                .map(|i| cap(*i))
            else {
                continue;
            };
            // Accessibility with every unlock *except* this one.
            let without: std::collections::BTreeSet<ObjectId> =
                (0..3).map(cap).filter(|c| *c != granted).collect();
            let r = mission.sweep(
                &without,
                &solution.placements,
                &std::collections::BTreeMap::new(),
            );
            let scope = mission
                .locations()
                .find(|(id, _)| id == loc)
                .unwrap()
                .1
                .scope;
            assert!(
                r.accessible(scope),
                "seed {seed}: {placed} sits somewhere needing {granted} to reach"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The dials
// ---------------------------------------------------------------------------------------------

#[test]
fn cycle_density_changes_how_much_the_world_loops() {
    // ⚠ This replaced two tests at M04a that profiled `Linearity::LINEAR` vs `Linearity::OPEN`.
    // `progression_locality` — half of what they measured — is a dial the design **refuses**: a door
    // states key-to-lock distance as a `MinDistanceFrom` constraint, so a dial would be a second way
    // to say the same thing. `cycle_density` survives, because no Actor can state *"this world has
    // many loops"* — that is genuinely the generator's to decide.
    let (g, spaces) = build_world(3, 4);

    let loops_at = |density: f64| {
        let mut total = 0usize;
        for seed in 0..12u64 {
            let (mission, _) = generate(&g, &spaces, density, 3, 0.6, seed).unwrap();
            total += mission.edges().iter().filter(|e| e.is_shortcut).count();
        }
        total
    };

    let chain = loops_at(0.0);
    let webbed = loops_at(1.0);
    assert_eq!(chain, 0, "at density 0 no shortcut may be added");
    assert!(
        webbed > chain,
        "density 1 must loop more than density 0 — {webbed} vs {chain}"
    );
}

#[test]
fn gating_creates_real_progression_structure() {
    let (g, spaces) = build_world(3, 4);

    // Ungated: everything is accessible immediately — a single sphere.
    let caps: Vec<ObjectId> = (0..3).map(cap).collect();
    let items: Vec<ObjectId> = (0..3).map(item).collect();
    let mut solver = Solver::new(&g);
    for i in 0..3 {
        solver = solver.with_grant(item(i), cap(i));
    }
    let open_world = mission_for(&g, &spaces);
    let flat = solver.fill(&open_world, &items, &Rng::new(1)).unwrap();
    assert_eq!(
        flat.depth(),
        1,
        "with no gates there is nothing to progress through"
    );

    // Gated: the world splits into spheres.
    let mut gated = mission_for(&g, &spaces);
    let rng = Rng::new(1);
    let count = solver.gate_edges(&mut gated, &caps, 0.8, &rng);
    assert!(count > 0, "the gating actually applied");
    let staged = solver.fill(&gated, &items, &rng).unwrap();
    assert!(
        staged.depth() > 1,
        "gating must produce progression, got {} spheres",
        staged.depth()
    );
    // Sphere 0 needs nothing; later spheres exist only because unlocks were found.
    assert!(!staged.accessibility.spheres[0].granted.is_empty() || staged.depth() == 1);
}

// ---------------------------------------------------------------------------------------------
// Determinism and diagnostics
// ---------------------------------------------------------------------------------------------

#[test]
fn generation_is_reproducible() {
    let (g, spaces) = build_world(3, 4);
    for seed in [1u64, 42, 0xDEAD] {
        let a = generate(&g, &spaces, 0.5, 3, 0.6, seed).unwrap();
        let b = generate(&g, &spaces, 0.5, 3, 0.6, seed).unwrap();
        assert_eq!(a.0, b.0, "the same seed must build the same graph");
        assert_eq!(a.1, b.1, "and place the same items");
    }
}

#[test]
fn different_seeds_explore_different_worlds() {
    let (g, spaces) = build_world(3, 4);
    let a = generate(&g, &spaces, 0.5, 3, 0.6, 1).unwrap().1;
    let b = generate(&g, &spaces, 0.5, 3, 0.6, 2).unwrap().1;
    assert_ne!(a.placements, b.placements);
}

#[test]
fn an_unsolvable_world_is_reported_rather_than_returned_broken() {
    // A sealed edge with the only location behind it. There is no arrangement, and no amount of
    // retrying invents one — so the solver must say so instead of returning an unfinishable world.
    let (g, spaces) = build_world(1, 3);
    let mut mission = MissionGraph::from_scopes(&g, spaces[0]);
    mission.gate_edge(0, Rule::Never);
    mission.add_location(
        LocationId(0),
        Location {
            scope: spaces[2],
            slot: 0,
        },
    );

    let solver = Solver::new(&g);
    let err = solver.fill(&mission, &[item(0)], &Rng::new(1)).unwrap_err();
    assert!(matches!(err, SolveError::NoAccessibleLocation { .. }));
}

#[test]
fn the_solution_explains_itself() {
    let (g, spaces) = build_world(3, 4);
    let (mission, solution) = generate(&g, &spaces, 0.5, 3, 0.6, 11).unwrap();

    assert_eq!(solution.traces.len(), 3);
    for t in &solution.traces {
        assert!(t.candidates > 0, "a placement always had somewhere to go");
        assert!(solution.placements.get(&t.location) == Some(&t.item));
    }
    assert!(solution.attempts >= 1);
    // Rules read back as something a human can check.
    for edge in mission.edges().iter().filter(|e| e.is_gated()) {
        assert!(!edge.rule.to_string().is_empty());
        assert!(!edge.rule.unlocks().is_empty());
    }
}
