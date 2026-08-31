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
    Linearity, LinearityOverride, LinearityResolver, Location, LocationId, MissionGraph, NodeGraph,
    NodeKind, NodeState, ObjectId, Rule, SolveError, Solver,
};
use cv_determinism::{Aabb, Rng, Vec3};

fn cap(i: usize) -> ObjectId {
    ObjectId::derived("token", &format!("cap_{i}"))
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
    resolver: &LinearityResolver,
    item_count: usize,
    gate_fraction: f64,
    seed: u64,
) -> Result<(MissionGraph, cv_core::Solution), SolveError> {
    let caps: Vec<ObjectId> = (0..item_count).map(cap).collect();
    let items: Vec<ObjectId> = (0..item_count).map(item).collect();

    let mut solver = Solver::new(g, resolver);
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
    // The property the milestone exists to establish, swept across shapes, dials and seeds.
    let shapes = [(2, 4), (3, 3), (4, 5), (1, 10)];
    let dials = [
        Linearity::LINEAR,
        Linearity::OPEN,
        Linearity::default(),
        Linearity::new(0.0, 1.0),
        Linearity::new(1.0, 0.0),
    ];

    let mut worlds = 0;
    let mut retried = 0;
    for (areas, per_area) in shapes {
        let (g, spaces) = build_world(areas, per_area);
        for dial in dials {
            let resolver = LinearityResolver::new(dial);
            for seed in 0..25u64 {
                let (_, solution) =
                    generate(&g, &spaces, &resolver, 3, 0.6, seed).unwrap_or_else(|e| {
                        panic!("{areas}x{per_area} dial {dial:?} seed {seed}: {e}")
                    });

                // Every token is obtainable...
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
    assert_eq!(worlds, 500);
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
    let resolver = LinearityResolver::new(Linearity::default());

    for seed in 0..40u64 {
        let (mission, solution) = generate(&g, &spaces, &resolver, 3, 0.7, seed).unwrap();
        for (loc, placed) in &solution.placements {
            let Some(granted) = [0, 1, 2]
                .iter()
                .find(|i| item(**i) == *placed)
                .map(|i| cap(*i))
            else {
                continue;
            };
            // Accessibility with every token *except* this one.
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
fn the_dials_produce_measurably_different_worlds() {
    let (g, spaces) = build_world(3, 5);

    let profile = |dial: Linearity| {
        let resolver = LinearityResolver::new(dial);
        let mut shortcuts = 0usize;
        let mut distance = 0u32;
        for seed in 0..25u64 {
            let (mission, solution) = generate(&g, &spaces, &resolver, 3, 0.6, seed).unwrap();
            shortcuts += mission.shortcut_count();
            distance += solution
                .traces
                .iter()
                .filter_map(|t| t.distance_to_lock)
                .sum::<u32>();
        }
        (shortcuts, distance)
    };

    let (linear_loops, linear_distance) = profile(Linearity::LINEAR);
    let (open_loops, open_distance) = profile(Linearity::OPEN);

    assert_eq!(linear_loops, 0, "a linear world has no shortcuts at all");
    assert!(open_loops > 0, "an open world loops");
    assert!(
        open_distance > linear_distance,
        "open worlds should scatter keys further from their locks ({open_distance} vs {linear_distance})"
    );
}

#[test]
fn a_linear_stretch_can_sit_inside_an_open_world() {
    // The mixing case: mostly non-linear, but one Area deliberately kept self-contained.
    let (g, spaces) = build_world(3, 4);
    let quiet_area = g.scope_of(spaces[4], NodeKind::Area).unwrap();

    let mut resolver = LinearityResolver::new(Linearity::OPEN);
    resolver.override_scope(quiet_area, LinearityOverride::cycles(0.0));

    let mut inside = 0usize;
    let mut outside = 0usize;
    for seed in 0..30u64 {
        let (mission, _) = generate(&g, &spaces, &resolver, 3, 0.5, seed).unwrap();
        for edge in mission.edges().iter().filter(|e| e.is_shortcut) {
            if g.scope_of(edge.from, NodeKind::Area) == Some(quiet_area) {
                inside += 1;
            } else {
                outside += 1;
            }
        }
    }
    assert_eq!(inside, 0, "the overridden Area stays loop-free");
    assert!(outside > 0, "while the rest of the world still loops");
}

#[test]
fn gating_creates_real_progression_structure() {
    let (g, spaces) = build_world(3, 4);
    let resolver = LinearityResolver::new(Linearity::default());

    // Ungated: everything is accessible immediately — a single sphere.
    let caps: Vec<ObjectId> = (0..3).map(cap).collect();
    let items: Vec<ObjectId> = (0..3).map(item).collect();
    let mut solver = Solver::new(&g, &resolver);
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
    // Sphere 0 needs nothing; later spheres exist only because tokens were found.
    assert!(!staged.accessibility.spheres[0].granted.is_empty() || staged.depth() == 1);
}

// ---------------------------------------------------------------------------------------------
// Determinism and diagnostics
// ---------------------------------------------------------------------------------------------

#[test]
fn generation_is_reproducible() {
    let (g, spaces) = build_world(3, 4);
    let resolver = LinearityResolver::new(Linearity::default());
    for seed in [1u64, 42, 0xDEAD] {
        let a = generate(&g, &spaces, &resolver, 3, 0.6, seed).unwrap();
        let b = generate(&g, &spaces, &resolver, 3, 0.6, seed).unwrap();
        assert_eq!(a.0, b.0, "the same seed must build the same graph");
        assert_eq!(a.1, b.1, "and place the same items");
    }
}

#[test]
fn different_seeds_explore_different_worlds() {
    let (g, spaces) = build_world(3, 4);
    let resolver = LinearityResolver::new(Linearity::default());
    let a = generate(&g, &spaces, &resolver, 3, 0.6, 1).unwrap().1;
    let b = generate(&g, &spaces, &resolver, 3, 0.6, 2).unwrap().1;
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

    let resolver = LinearityResolver::new(Linearity::default());
    let solver = Solver::new(&g, &resolver);
    let err = solver.fill(&mission, &[item(0)], &Rng::new(1)).unwrap_err();
    assert!(matches!(err, SolveError::NoAccessibleLocation { .. }));
}

#[test]
fn the_solution_explains_itself() {
    let (g, spaces) = build_world(3, 4);
    let resolver = LinearityResolver::new(Linearity::new(0.25, 0.4));
    let (mission, solution) = generate(&g, &spaces, &resolver, 3, 0.6, 11).unwrap();

    assert_eq!(solution.traces.len(), 3);
    for t in &solution.traces {
        assert!(t.candidates > 0, "a placement always had somewhere to go");
        assert_eq!(
            t.locality, 0.25,
            "the dial in force is recorded, not assumed"
        );
        assert!(solution.placements.get(&t.location) == Some(&t.item));
    }
    assert!(solution.attempts >= 1);
    // Rules read back as something a human can check.
    for edge in mission.edges().iter().filter(|e| e.is_gated()) {
        assert!(!edge.rule.to_string().is_empty());
        assert!(!edge.rule.tokens().is_empty());
    }
}
