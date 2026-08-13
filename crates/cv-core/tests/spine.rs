//! M10a exit criteria: an opt-in macro-structure that is **guaranteed** when present.
//!
//! Three claims, and the third is the one that makes the feature worth having:
//!
//! 1. **Opt-in is real, not nominal.** Registering no spine must reproduce M09/M10 *byte for byte*.
//!    A feature that quietly perturbs everyone who is not using it is not opt-in.
//! 2. **The dial cannot erode a contract.** `Required` holds at `adherence = 0.0`, across seeds,
//!    every time — while the softer tiers do vary as documented, and every relaxation is reported.
//! 3. **A spine composes with the rest of the pipeline.** Fill still solves, the un-softlockable pass
//!    still holds, and a host can find the guaranteed rooms in the descriptor.
//!
//! (2) is meaningless without (3): a spine that guarantees a boss room in a world the player cannot
//! finish has kept the letter of the promise and broken the point of it.

use cv_core::{
    AdaptiveRange, ContentKind, ContentPool, ContentRegistry, CountRule, Schedule, ScheduleBook,
    Scheduler, SlotRule,
};
use cv_core::{
    CapabilityRef, Coverage, DescriptorBuilder, Fingerprint, GrantSpec, Handle, Linearity,
    LinearityResolver, Location, LocationId, MissionGraph, Node, NodeGraph, NodeKind, NodeState,
    ObjectId, SlotRole, SoftlockAnalyzer, Solver, SpineError, SpineInstantiator, SpineSegment,
    SpineSlot, SpineSlotTag, SpineTemplate, Strictness,
};
use cv_determinism::{Aabb, Rng, Vec3};
use std::collections::{BTreeMap, BTreeSet};

fn cap(name: &str) -> ObjectId {
    ObjectId::derived("capability", name)
}
fn item(name: &str) -> ObjectId {
    ObjectId::derived("item", name)
}
fn actor(name: &str) -> ObjectId {
    ObjectId::derived("actor", name)
}
fn spine_id(name: &str) -> ObjectId {
    ObjectId::derived("spine", name)
}

/// `reaches` Reaches, each one Area of `per` chained Spaces, all Realized.
fn world(reaches: usize, per: usize) -> (NodeGraph, Vec<Handle<Node>>) {
    let mut g = NodeGraph::new(1.0, 1);
    let mut spaces = Vec::new();
    for r in 0..reaches {
        let reach = g.add_child(g.root(), format!("reach_{r}")).unwrap();
        let area = g.add_child(reach, format!("area_{r}")).unwrap();
        let rooms: Vec<_> = (0..per)
            .map(|s| g.add_child(area, format!("space_{r}_{s}")).unwrap())
            .collect();
        for w in rooms.windows(2) {
            g.connect(w[0], w[1]).unwrap();
        }
        if let (Some(prev), Some(first)) = (spaces.last().copied(), rooms.first().copied()) {
            g.connect(prev, first).unwrap();
        }
        spaces.extend(rooms);
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
// The three worked examples from the design
// ---------------------------------------------------------------------------------------------

/// A per-Reach loop: `start → <anything> → capstone(boss) → terminal(sanctum)`, where the terminal
/// must be an exit of the capstone. Every slot `Required` — the pattern *is* the game's rhythm.
fn per_reach_loop() -> SpineTemplate {
    SpineTemplate::new(spine_id("reach_loop"), NodeKind::Reach)
        .slot(SpineSlot::new("start").role(SlotRole::Start))
        .slot(SpineSlot::new("capstone").must_contain([actor("boss")]))
        .slot(
            SpineSlot::new("terminal")
                .role(SlotRole::Goal)
                .must_contain([actor("waypoint")])
                .adjacent_to("capstone"),
        )
        .segment(SpineSegment::new(
            "start",
            "capstone",
            AdaptiveRange::new(1, 4),
        ))
        .segment(SpineSegment::direct("capstone", "terminal"))
}

/// A dungeon: `start → <anything> → precursor → <gated anything> → capstone`. The precursor grants
/// *something* the generator picks, and both the path onward and the boss require that same thing.
fn dungeon() -> SpineTemplate {
    SpineTemplate::new(spine_id("dungeon"), NodeKind::Area)
        .slot(SpineSlot::new("start").role(SlotRole::Start))
        .slot(SpineSlot::new("precursor").grants(GrantSpec::any_of([cap("hook"), cap("bombs")])))
        .slot(
            SpineSlot::new("capstone")
                .role(SlotRole::Goal)
                .must_contain([actor("boss")])
                .requires(CapabilityRef::GrantedBy("precursor".into())),
        )
        .segment(SpineSegment::new(
            "start",
            "precursor",
            AdaptiveRange::new(1, 3),
        ))
        .segment(
            SpineSegment::new("precursor", "capstone", AdaptiveRange::new(1, 3))
                .gated_by(CapabilityRef::GrantedBy("precursor".into())),
        )
}

/// A loose spine: one anchor the dev insists on, everything else advisory.
fn loose() -> SpineTemplate {
    SpineTemplate::new(spine_id("loose"), NodeKind::Reach)
        .adherence(0.4)
        .slot(SpineSlot::new("start"))
        .slot(SpineSlot::new("landmark").strictness(Strictness::Preferred))
        .slot(SpineSlot::new("flourish").strictness(Strictness::Optional))
        .segment(SpineSegment::new(
            "start",
            "landmark",
            AdaptiveRange::new(1, 3),
        ))
}

fn registry() -> ContentRegistry {
    let mut r = ContentRegistry::new();
    r.register(ContentKind::Actor, "boss", 1).unwrap();
    r.register(ContentKind::Actor, "waypoint", 1).unwrap();
    r.register(ContentKind::Capability, "hook", 1).unwrap();
    r.register(ContentKind::Capability, "bombs", 1).unwrap();
    r
}

// ---------------------------------------------------------------------------------------------
// 1. Opt-in is real
// ---------------------------------------------------------------------------------------------

#[test]
fn registering_no_spine_reproduces_the_pipeline_exactly() {
    // The strongest form of "opt-in": run the *whole* solve twice, once with the spine pass wired in
    // but empty. Anything the pass touched — an extra edge, one RNG draw — would show up here.
    let (g, spaces) = world(2, 5);
    let resolver = LinearityResolver::new(Linearity::new(0.5, 0.3));
    let solver = Solver::new(&g, &resolver).with_grant(item("dash_item"), cap("dash"));

    let run = |use_pass: bool| {
        let mut mission = MissionGraph::from_scopes(&g, spaces[0]);
        for (i, s) in spaces.iter().enumerate() {
            mission.add_location(LocationId(i as u32), Location { scope: *s, slot: 0 });
        }
        let rng = Rng::new(0xC0FFEE);
        if use_pass {
            let instances = SpineInstantiator::new(&g).instantiate(&mut mission, &rng);
            assert!(instances.is_empty());
        }
        solver.add_cycles(&mut mission, &rng);
        let solution = solver.fill(&mission, &[item("dash_item")], &rng).unwrap();
        (mission, solution)
    };

    let (m_without, s_without) = run(false);
    let (m_with, s_with) = run(true);
    assert_eq!(m_without, m_with, "an empty spine pass must change nothing");
    assert_eq!(s_without, s_with, "and must not consume a single draw");
}

// ---------------------------------------------------------------------------------------------
// 2. The dial cannot erode a contract
// ---------------------------------------------------------------------------------------------

#[test]
fn a_required_spine_holds_across_every_seed() {
    // The whole promise, stated as a property: not "usually", not "at high adherence" — every time.
    let (g, _) = world(3, 7);
    for seed in 0..64u64 {
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(per_reach_loop())
            .instantiate(&mut mission, &Rng::new(seed));

        assert_eq!(instances.len(), 3, "seed {seed}: one per Reach");
        for instance in &instances {
            assert!(
                instance.relaxations.is_empty(),
                "seed {seed}: a Required slot was relaxed: {:?}",
                instance.relaxations
            );
            for slot in ["start", "capstone", "terminal"] {
                assert!(
                    instance.scope_of(slot).is_some(),
                    "seed {seed}: {slot} missing"
                );
            }
            let capstone = instance.scope_of("capstone").unwrap();
            let terminal = instance.scope_of("terminal").unwrap();
            assert!(
                mission.connects(capstone, terminal),
                "seed {seed}: the terminal must be an exit of the capstone"
            );
        }
    }
}

#[test]
fn adherence_zero_does_not_weaken_a_required_spine() {
    // The test that says the dial is safe: turning it to nothing must leave the contract intact.
    let (g, _) = world(2, 7);
    for adherence in [0.0, 0.25, 0.5, 1.0] {
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(per_reach_loop().adherence(adherence))
            .instantiate(&mut mission, &Rng::new(7));
        for instance in &instances {
            assert!(
                instance.relaxations.is_empty(),
                "adherence {adherence} relaxed a Required slot"
            );
            assert_eq!(instance.assignments.len(), 3);
        }
    }
}

#[test]
fn soft_tiers_vary_with_adherence_and_every_drop_is_reported() {
    let (g, _) = world(1, 7);
    let outcome = |adherence: f64| {
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(loose().adherence(adherence))
            .instantiate(&mut mission, &Rng::new(1));
        let i = &instances[0];
        (i.assignments.len(), i.relaxations.len())
    };
    // Slots kept, slots dropped — and the two always sum to the declared three, which is what
    // "nothing is loose by accident" means in practice.
    assert_eq!(outcome(1.0), (3, 0));
    assert_eq!(outcome(0.4), (2, 1));
    assert_eq!(outcome(0.0), (1, 2));
    for adherence in [0.0, 0.4, 1.0] {
        let (kept, dropped) = outcome(adherence);
        assert_eq!(kept + dropped, 3, "an unaccounted-for slot is the bug");
    }
}

// ---------------------------------------------------------------------------------------------
// 3. It composes with the rest of the pipeline
// ---------------------------------------------------------------------------------------------

#[test]
fn a_spined_world_still_solves_and_stays_un_softlockable() {
    // A guarantee that produces an unfinishable world has kept the letter and broken the point.
    //
    // One-way commits are switched **on** deliberately. Without them no hazard is structurally
    // possible and this test would assert safety about a world that could not have been unsafe —
    // the M10 lesson, which cost a rewrite of that milestone's fixture.
    let (g, spaces) = world(2, 8);
    let resolver = LinearityResolver::new(Linearity::new(0.4, 0.4));
    let solver = Solver::new(&g, &resolver)
        .with_grant(item("hook_item"), cap("hook"))
        .with_grant(item("bomb_item"), cap("bombs"));
    let mut seeds_with_a_hazard = 0;

    for seed in 0..16u64 {
        let rng = Rng::new(seed);
        let mut mission = MissionGraph::from_scopes(&g, spaces[0]);
        let instances = SpineInstantiator::new(&g)
            .with_template(per_reach_loop())
            .instantiate(&mut mission, &rng);
        assert_eq!(instances.len(), 2);

        // The spine's guarantee is what makes the goal nameable at all: "the last terminal" is a
        // thing a host can *say* only because the slot is promised to exist.
        let goal = instances
            .last()
            .and_then(|i| i.scope_of("terminal"))
            .expect("the terminal is guaranteed");
        mission.set_goal(goal);

        for (i, s) in spaces.iter().enumerate() {
            mission.add_location(LocationId(i as u32), Location { scope: *s, slot: 0 });
        }
        solver.gate_edges(&mut mission, &[cap("hook"), cap("bombs")], 0.3, &rng);
        solver.add_one_way_commits(&mut mission, 0.35, &rng);

        let solution = solver
            .fill(&mission, &[item("hook_item"), item("bomb_item")], &rng)
            .unwrap_or_else(|e| panic!("seed {seed}: spined world did not solve: {e}"));

        // The guaranteed rooms survive gating — they are still reachable in the solved world.
        for instance in &instances {
            for slot in ["capstone", "terminal"] {
                let scope = instance.scope_of(slot).unwrap();
                assert!(
                    solution.reachability.reaches(scope),
                    "seed {seed}: the {slot} must be reachable, not merely present"
                );
            }
        }

        let analysis =
            SoftlockAnalyzer::new(&mission, &solution.placements, solver.grants()).analyze();
        assert_eq!(
            analysis.limit, None,
            "seed {seed}: the analysis must complete"
        );
        if !analysis.hazards.is_empty() {
            seeds_with_a_hazard += 1;
            analysis.repair(&mut mission);
            let after =
                SoftlockAnalyzer::new(&mission, &solution.placements, solver.grants()).analyze();
            assert!(
                after.is_un_softlockable(),
                "seed {seed}: {} hazards survived repair",
                after.hazards.len()
            );
        }

        // Repair changes edges; the guarantee must be unaffected by it.
        for instance in &instances {
            let capstone = instance.scope_of("capstone").unwrap();
            let terminal = instance.scope_of("terminal").unwrap();
            assert!(
                mission.connects(capstone, terminal),
                "seed {seed}: repair broke the capstone→terminal guarantee"
            );
        }
    }

    // Without this the sweep could pass by never having produced a hazard at all — a green light
    // from a detector that was never asked a hard question.
    assert!(
        seeds_with_a_hazard > 0,
        "no seed produced a hazard; the safety assertions proved nothing"
    );
}

#[test]
fn all_three_worked_examples_generate() {
    let registry = registry();
    for (label, template, kind, scopes) in [
        ("per-Reach loop", per_reach_loop(), NodeKind::Reach, 8usize),
        ("dungeon", dungeon(), NodeKind::Area, 8),
        ("loose", loose(), NodeKind::Reach, 8),
    ] {
        let validation = template.validate(&registry, scopes as u32);
        assert!(
            validation.is_ok(),
            "{label} failed validation: {:?}",
            validation.errors
        );

        let (g, _) = world(2, scopes);
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(template.clone())
            .instantiate(&mut mission, &Rng::new(11));
        assert!(!instances.is_empty(), "{label} produced no instances");

        // Every `Required` slot is present in every instance, whatever else varied.
        for instance in &instances {
            for slot in &template.slots {
                if slot.effective_strictness(template.strictness).is_required() {
                    assert!(
                        instance.scope_of(&slot.name).is_some(),
                        "{label}: required slot {} missing",
                        slot.name
                    );
                }
            }
        }
        let _ = kind;
    }
}

// ---------------------------------------------------------------------------------------------
// Degree: "at least X connected", and the dead end that has to *stay* one
// ---------------------------------------------------------------------------------------------

/// The shape a rest-sanctum loop wants: a boss chamber with several ways in, one of which leads to a
/// sanctum that is directly attached and goes nowhere else.
fn sanctum_loop() -> SpineTemplate {
    SpineTemplate::new(spine_id("sanctum_loop"), NodeKind::Reach)
        .slot(SpineSlot::new("start").role(SlotRole::Start))
        .slot(SpineSlot::new("capstone").min_degree(3))
        .slot(
            SpineSlot::new("terminal")
                .role(SlotRole::Goal)
                .adjacent_to("capstone")
                .dead_end(),
        )
        .segment(SpineSegment::new(
            "start",
            "capstone",
            AdaptiveRange::new(1, 4),
        ))
        .segment(SpineSegment::direct("capstone", "terminal"))
}

#[test]
fn a_dead_end_slot_survives_free_form_growth_and_maximum_cycle_density() {
    // The whole point of a cap: it has to beat the dial, not merely precede it. `cycle_density = 1.0`
    // is the most aggressive setting there is, and it must not touch the sanctum.
    let (g, spaces) = world(1, 9);
    let rng = Rng::new(7);

    // The documented order — spine seeds an empty graph, free-form adjacency pours in around it.
    let mut mission = MissionGraph::new(spaces[0]);
    let instances = SpineInstantiator::new(&g)
        .with_template(sanctum_loop())
        .instantiate(&mut mission, &rng);
    mission.connect_scopes(&g);

    let instance = &instances[0];
    assert!(
        instance.relaxations.is_empty(),
        "{:?}",
        instance.relaxations
    );
    let capstone = instance.scope_of("capstone").unwrap();
    let terminal = instance.scope_of("terminal").unwrap();

    assert!(
        mission.connects(capstone, terminal),
        "the sanctum must be an exit of the capstone"
    );
    assert!(
        mission.degree(capstone) >= 3,
        "the capstone keeps its free-form exits"
    );
    assert_eq!(mission.degree(terminal), 1, "the sanctum is a dead end");

    // Now turn the cycle dial to maximum and confirm the cap holds where discipline would not.
    let resolver = LinearityResolver::new(Linearity::new(0.5, 1.0));
    let added = Solver::new(&g, &resolver).add_cycles(&mut mission, &rng);
    assert!(
        added > 0,
        "the dial must actually have done something, or this proves nothing"
    );
    assert_eq!(
        mission.degree(terminal),
        1,
        "cycle_density broke the dead end — a dial beat a guarantee"
    );
    assert!(
        mission
            .edges()
            .iter()
            .filter(|e| e.from == terminal || e.to == terminal)
            .all(|e| e.from == capstone || e.to == capstone),
        "the sanctum's one connection must be the capstone itself"
    );
    // And the capstone, which declared only a floor, was free to grow.
    assert!(
        mission.degree(capstone) > 3,
        "a floor must not act as a ceiling"
    );
}

#[test]
fn a_cap_declared_too_late_is_reported_rather_than_silently_missed() {
    // Caps look forward. Seeding into a graph that already has adjacency cannot retroactively prune —
    // tearing edges out would invalidate every index the solver and softlock pass hold — so the honest
    // move is to say so.
    let (g, spaces) = world(1, 9);
    let mut mission = MissionGraph::from_scopes(&g, spaces[0]); // adjacency already present
    let instances = SpineInstantiator::new(&g)
        .with_template(sanctum_loop())
        .instantiate(&mut mission, &Rng::new(7));

    let instance = &instances[0];
    assert!(
        instance
            .relaxations
            .iter()
            .any(|r| r.slot == "terminal" && r.reason.contains("max_degree")),
        "an unenforceable cap must be reported: {:?}",
        instance.relaxations
    );
    // And the guidance points at the fix rather than just naming the problem.
    let text = instance.relaxations[0].to_string();
    assert!(text.contains("connect_scopes"), "{text}");
}

#[test]
fn a_ceiling_that_contradicts_the_same_slots_own_demands_is_an_authoring_error() {
    // A dead end asked to touch two *other* slots is a contradiction; picking a winner silently would
    // mean quietly dropping something the dev wrote.
    let contradiction = SpineTemplate::new(spine_id("contradiction"), NodeKind::Reach)
        .slot(SpineSlot::new("a"))
        .slot(SpineSlot::new("b"))
        .slot(SpineSlot::new("hub"))
        .slot(
            SpineSlot::new("sanctum")
                .dead_end()
                .adjacent_to("a")
                .adjacent_to("b"),
        );
    let v = contradiction.validate(&ContentRegistry::new(), 20);
    assert!(!v.is_ok());
    let text = v
        .errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ");
    assert!(text.contains("caps connections at 1"), "{text}");
    assert!(text.contains("route in"), "{text}");

    // min above max is the other contradiction, and is caught by the same rule.
    let inverted = SpineTemplate::new(spine_id("inverted"), NodeKind::Reach)
        .slot(SpineSlot::new("start"))
        .slot(SpineSlot::new("odd").min_degree(4).max_degree(2));
    assert!(inverted
        .validate(&ContentRegistry::new(), 20)
        .errors
        .iter()
        .any(|e| matches!(e, SpineError::DegreeContradiction { .. })));

    // The CrawlStar shape itself is *not* a contradiction — one route in, capped at one.
    assert!(sanctum_loop().validate(&registry(), 9).is_ok());
}

#[test]
fn a_dead_end_sanctum_does_not_strand_anyone() {
    // A cul-de-sac is only safe because you can walk back out. Worth pinning: this is exactly the
    // shape that *would* be a trap if the connecting edge were ever made one-way.
    let (g, spaces) = world(1, 9);
    let rng = Rng::new(3);
    let mut mission = MissionGraph::new(spaces[0]);
    let instances = SpineInstantiator::new(&g)
        .with_template(sanctum_loop())
        .instantiate(&mut mission, &rng);
    mission.connect_scopes(&g);

    let instance = &instances[0];
    let terminal = instance.scope_of("terminal").unwrap();
    mission.set_goal(terminal);
    for (i, s) in spaces.iter().enumerate() {
        mission.add_location(LocationId(i as u32), Location { scope: *s, slot: 0 });
    }

    let resolver = LinearityResolver::new(Linearity::new(0.4, 0.4));
    let solver = Solver::new(&g, &resolver).with_grant(item("hook_item"), cap("hook"));
    solver.add_one_way_commits(&mut mission, 0.4, &rng);
    let solution = solver.fill(&mission, &[item("hook_item")], &rng).unwrap();

    let analysis = SoftlockAnalyzer::new(&mission, &solution.placements, solver.grants()).analyze();
    let repaired = analysis.repair(&mut mission);
    let after = SoftlockAnalyzer::new(&mission, &solution.placements, solver.grants()).analyze();
    assert!(
        after.is_un_softlockable(),
        "{} hazards remain after {} repairs",
        after.hazards.len(),
        repaired.len()
    );
    // Repair adds no edges, so the cap is still intact afterwards.
    assert_eq!(
        mission.degree(terminal),
        1,
        "repair must not have widened the dead end"
    );
}

// ---------------------------------------------------------------------------------------------
// Segments: "anything can go between these two, you decide"
// ---------------------------------------------------------------------------------------------

/// A world whose Spaces are richly interconnected rather than a single chain, so the algorithm has
/// something to branch *with*. A corridor graph cannot branch however free the segment is.
fn braided_world(spaces_per_reach: usize) -> (NodeGraph, Vec<Handle<Node>>) {
    let mut g = NodeGraph::new(1.0, 1);
    let reach = g.add_child(g.root(), "reach").unwrap();
    let area = g.add_child(reach, "area").unwrap();
    let rooms: Vec<_> = (0..spaces_per_reach)
        .map(|s| g.add_child(area, format!("space_{s}")).unwrap())
        .collect();
    for w in rooms.windows(2) {
        g.connect(w[0], w[1]).unwrap();
    }
    // Extra spatial links so branching is available to whoever wants it.
    for w in rooms.windows(3) {
        g.connect(w[0], w[2]).unwrap();
    }
    for h in g.walk() {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
            .unwrap();
    }
    for h in g.walk() {
        g.advance(h, NodeState::Realized).unwrap();
    }
    (g, rooms)
}

#[test]
fn a_free_segment_takes_whatever_the_instance_can_spare() {
    // "I don't care how long" stated explicitly, rather than faked with a big AdaptiveRange.
    let spine = SpineTemplate::new(spine_id("free"), NodeKind::Reach)
        .slot(SpineSlot::new("start").role(SlotRole::Start))
        .slot(SpineSlot::new("finish").role(SlotRole::Goal))
        .segment(SpineSegment::free("start", "finish"));
    assert!(spine.segments[0].is_free());
    // Two slots plus everything else as connective tissue, at any capacity.
    let kept = spine.kept_slots();
    assert_eq!(spine.segment_lengths(&kept, 12), vec![10]);
    assert_eq!(spine.segment_lengths(&kept, 2), vec![0]);
    // The floor is just the slots — a free segment demands nothing.
    assert_eq!(spine.required_minimum(), 2);

    // Several free segments split the surplus evenly rather than overflowing the apportionment.
    let three = SpineTemplate::new(spine_id("free3"), NodeKind::Reach)
        .slot(SpineSlot::new("a"))
        .slot(SpineSlot::new("b"))
        .slot(SpineSlot::new("c"))
        .segment(SpineSegment::free("a", "b"))
        .segment(SpineSegment::free("b", "c"));
    let kept = three.kept_slots();
    assert_eq!(three.segment_lengths(&kept, 13), vec![5, 5]);
}

#[test]
fn a_free_segment_may_branch_and_reconverge() {
    // The claim the API makes: a segment guarantees *a* route, not the *only* route. If the interior
    // could only ever be a corridor, "anything can go here" would be false advertising.
    let (g, spaces) = braided_world(10);
    let rng = Rng::new(21);
    let spine = SpineTemplate::new(spine_id("open"), NodeKind::Reach)
        .slot(SpineSlot::new("start").role(SlotRole::Start))
        .slot(SpineSlot::new("finish").role(SlotRole::Goal))
        .segment(SpineSegment::free("start", "finish"));

    let mut mission = MissionGraph::new(spaces[0]);
    let instances = SpineInstantiator::new(&g)
        .with_template(spine)
        .instantiate(&mut mission, &rng);
    mission.connect_scopes(&g);

    let start = instances[0].scope_of("start").unwrap();
    let finish = instances[0].scope_of("finish").unwrap();
    assert!(
        mission.distances_from(start).contains_key(&finish),
        "the guaranteed route must exist before anything else is asserted"
    );

    // Nothing inside a segment is degree-capped — that is what leaves the shape open.
    for s in &spaces {
        assert!(
            mission.degree_cap(*s).is_none(),
            "a segment interior must not be capped; only slots that asked are"
        );
    }

    let resolver = LinearityResolver::new(Linearity::new(0.5, 1.0));
    Solver::new(&g, &resolver).add_cycles(&mut mission, &rng);

    // Branching: some room has three or more ways out.
    let branchiest = spaces.iter().map(|s| mission.degree(*s)).max().unwrap();
    assert!(
        branchiest >= 3,
        "no room branched; the segment interior was a corridor after all (max degree {branchiest})"
    );

    // Reconvergence: more edges than a tree over the same rooms means at least one loop exists, so
    // there is genuinely more than one way through.
    let interior: BTreeSet<_> = spaces.iter().copied().collect();
    let edges = mission
        .edges()
        .iter()
        .filter(|e| interior.contains(&e.from) && interior.contains(&e.to))
        .count();
    assert!(
        edges >= interior.len(),
        "a tree has {} edges for {} rooms; found {edges}, so nothing reconverged",
        interior.len() - 1,
        interior.len()
    );
}

#[test]
fn segment_latitude_is_one_mechanism_with_three_settings() {
    // direct / bounded / free are the same statement at different strengths, not three features.
    let kept_of = |seg: SpineSegment| {
        let t = SpineTemplate::new(spine_id("lat"), NodeKind::Reach)
            .slot(SpineSlot::new("a"))
            .slot(SpineSlot::new("b"))
            .segment(seg);
        let kept = t.kept_slots();
        (t.segment_lengths(&kept, 10)[0], t.required_minimum())
    };
    assert_eq!(
        kept_of(SpineSegment::direct("a", "b")),
        (0, 2),
        "no latitude"
    );
    assert_eq!(
        kept_of(SpineSegment::new("a", "b", AdaptiveRange::new(2, 4))),
        (4, 4),
        "bounded: grows to its ceiling, floors at its minimum"
    );
    assert_eq!(
        kept_of(SpineSegment::free("a", "b")),
        (8, 2),
        "free: takes the rest"
    );
}

// ---------------------------------------------------------------------------------------------
// Emptiness: rooms whose interior belongs to the host
// ---------------------------------------------------------------------------------------------

/// A room the generator leaves alone entirely: reached from one place, going nowhere, holding nothing
/// it chose. What the host puts inside is not the pipeline's business.
fn host_owned_room() -> SpineTemplate {
    SpineTemplate::new(spine_id("host_owned"), NodeKind::Reach)
        .slot(SpineSlot::new("origin").role(SlotRole::Start))
        .slot(SpineSlot::new("capstone").must_contain([actor("boss")]))
        .slot(
            SpineSlot::new("refuge")
                .adjacent_to("capstone")
                .dead_end()
                .empty(),
        )
        .segment(SpineSegment::new(
            "origin",
            "capstone",
            AdaptiveRange::new(1, 4),
        ))
        .segment(SpineSegment::direct("capstone", "refuge"))
}

#[test]
fn an_empty_slot_refuses_item_locations_however_many_passes_try() {
    // "Nothing to obtain in this room" has to hold against callers that do not know why, so the graph
    // refuses rather than each pass remembering to ask.
    let (g, spaces) = world(1, 9);
    let mut mission = MissionGraph::new(spaces[0]);
    let instances = SpineInstantiator::new(&g)
        .with_template(host_owned_room())
        .instantiate(&mut mission, &Rng::new(4));
    mission.connect_scopes(&g);

    let refuge = instances[0].scope_of("refuge").unwrap();
    assert!(mission.excludes_content(refuge));
    assert_eq!(instances[0].empty_scopes(), &[refuge]);

    // Every scope gets offered a location; only the empty one declines.
    let mut accepted = 0;
    for (i, s) in spaces.iter().enumerate() {
        if mission.add_location(LocationId(i as u32), Location { scope: *s, slot: 0 }) {
            accepted += 1;
        }
    }
    assert_eq!(accepted, spaces.len() - 1, "exactly one scope refused");
    assert!(
        mission.locations().all(|(_, l)| l.scope != refuge),
        "nothing may be findable in a room declared empty"
    );

    // And it is still a normal part of the world otherwise — reachable, connected, just not filled.
    let capstone = instances[0].scope_of("capstone").unwrap();
    assert!(mission.connects(capstone, refuge));
    assert_eq!(mission.degree(refuge), 1);
}

#[test]
fn an_empty_slot_is_skipped_by_scheduling_too() {
    // L1 plans against the scope graph and never sees the mission graph, so emptiness has to be
    // handed to it separately — a room with no item locations could still be handed scenery.
    let (g, spaces) = world(1, 9);
    let mut mission = MissionGraph::new(spaces[0]);
    let instances = SpineInstantiator::new(&g)
        .with_template(host_owned_room())
        .instantiate(&mut mission, &Rng::new(4));
    let refuge = instances[0].scope_of("refuge").unwrap();

    let mut registry = ContentRegistry::new();
    let statue = registry.register(ContentKind::Actor, "statue", 1).unwrap();
    let mut book = ScheduleBook::new();
    book.set(statue, Schedule::always());
    let pool = ContentPool::resolve(&registry, &book);

    let plan_for = |excluding: Vec<_>| {
        Scheduler::new(&g, &pool)
            .with_rule(SlotRule::new(NodeKind::Space, CountRule::fixed(2)))
            .excluding(excluding)
            .plan(&Rng::new(1))
    };

    let unfiltered = plan_for(Vec::new());
    let filtered = plan_for(instances[0].empty_scopes().to_vec());
    assert_eq!(
        filtered.len(),
        unfiltered.len() - 1,
        "the empty scope must drop out of the plan"
    );
    assert!(
        unfiltered.slots().iter().any(|s| s.scope == refuge),
        "the fixture must actually have planned for it, or this proves nothing"
    );
    assert!(
        !filtered.slots().iter().any(|s| s.scope == refuge),
        "an excluded scope must be absent from the plan, not present with a target of zero"
    );
}

#[test]
fn an_empty_slot_told_to_hold_something_is_an_authoring_error() {
    // Two of the dev's own statements contradict; picking a winner silently would drop one.
    let contradiction = SpineTemplate::new(spine_id("contradiction"), NodeKind::Reach)
        .slot(SpineSlot::new("start"))
        .slot(
            SpineSlot::new("refuge")
                .empty()
                .must_contain([actor("boss")]),
        );
    let v = contradiction.validate(&registry(), 10);
    assert!(!v.is_ok());
    let text = v
        .errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ");
    assert!(text.contains("declared empty"), "{text}");
    assert!(text.contains("must_contain"), "{text}");

    // A grant is the same contradiction: something has to be *there* to grant it.
    let granting = SpineTemplate::new(spine_id("granting"), NodeKind::Reach)
        .slot(SpineSlot::new("start"))
        .slot(
            SpineSlot::new("refuge")
                .empty()
                .grants(GrantSpec::any_of([cap("hook")])),
        );
    assert!(granting
        .validate(&registry(), 10)
        .errors
        .iter()
        .any(|e| matches!(e, SpineError::EmptyContradiction { .. })));

    // An empty room that is merely empty is fine.
    assert!(host_owned_room().validate(&registry(), 9).is_ok());
}

// ---------------------------------------------------------------------------------------------
// Roles: the two positions the core has an opinion about
// ---------------------------------------------------------------------------------------------

#[test]
fn a_world_has_a_start_whether_or_not_a_spine_declares_one() {
    // The start is not a spine concept — the mission graph cannot be built without one. A spine slot
    // relocates it onto a guaranteed room; it never introduces it.
    let (g, spaces) = world(2, 5);
    let bare = MissionGraph::from_scopes(&g, spaces[0]);
    assert_eq!(bare.start(), spaces[0], "no spine, still a start");

    let mut mission = MissionGraph::new(spaces[0]);
    let instances = SpineInstantiator::new(&g)
        .with_template(host_owned_room())
        .instantiate(&mut mission, &Rng::new(2));
    assert_eq!(
        mission.start(),
        instances[0].scope_of("origin").unwrap(),
        "a Start slot must *become* the start, not merely claim to be"
    );
}

#[test]
fn a_repeating_spine_yields_one_start_and_one_goal() {
    // A per-Reach spine declares its roles once per Reach; the world has exactly one of each. First
    // start, last goal — the only reading that stays coherent as Reaches are added.
    let (g, spaces) = world(3, 6);
    let template = SpineTemplate::new(spine_id("per_reach"), NodeKind::Reach)
        .slot(SpineSlot::new("origin").role(SlotRole::Start))
        .slot(SpineSlot::new("finish").role(SlotRole::Goal))
        .segment(SpineSegment::new(
            "origin",
            "finish",
            AdaptiveRange::new(1, 2),
        ));

    let mut mission = MissionGraph::new(spaces[0]);
    let instances = SpineInstantiator::new(&g)
        .with_template(template)
        .instantiate(&mut mission, &Rng::new(6));
    assert_eq!(instances.len(), 3);

    assert_eq!(
        mission.start(),
        instances[0].start.unwrap(),
        "the world begins in the first covered Reach"
    );
    assert_eq!(
        mission.goal(),
        Some(instances[2].goal.unwrap()),
        "and completes in the last"
    );
    // Not merely "some instance won" — the middle Reach must have claimed neither.
    assert_ne!(mission.start(), instances[1].start.unwrap());
    assert_ne!(mission.goal(), Some(instances[1].goal.unwrap()));
}

#[test]
fn a_symbolic_grant_resolves_to_one_capability_everywhere_it_is_named() {
    // The dungeon pattern's load-bearing trick: three references, one resolution.
    let (g, _) = world(1, 10);
    let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
    let instances = SpineInstantiator::new(&g)
        .with_template(dungeon())
        .instantiate(&mut mission, &Rng::new(5));

    let instance = &instances[0];
    let granted = instance.granted_by("precursor").expect("precursor grants");
    assert!(granted == cap("hook") || granted == cap("bombs"));
    assert!(
        mission
            .edges()
            .iter()
            .any(|e| e.rule.capabilities().contains(&granted)),
        "the gated segment must use exactly what the precursor granted"
    );
}

// ---------------------------------------------------------------------------------------------
// Validation is the feature: failure belongs at authoring time
// ---------------------------------------------------------------------------------------------

#[test]
fn an_impossible_spine_fails_with_the_arithmetic_shown() {
    // The dev must learn *why*, in numbers they can act on — not receive a world missing a room.
    let template = per_reach_loop();
    let v = template.validate(&registry(), 2);
    assert!(!v.is_ok());
    let text = v
        .errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ");
    assert!(text.contains("needs 4 scopes"), "{text}");
    assert!(text.contains("2 are available"), "{text}");

    // And the same spine passes the moment the budget can hold it.
    assert!(template.validate(&registry(), 4).is_ok());
}

#[test]
fn a_missing_required_actor_is_an_error_but_a_missing_preferred_one_is_a_warning() {
    // Only `Required` can block. That distinction is the entire point of the tiers.
    let empty = ContentRegistry::new();
    let hard = per_reach_loop().validate(&empty, 10);
    assert!(hard
        .errors
        .iter()
        .any(|e| matches!(e, SpineError::ContentUnavailable { .. })));

    let soft = per_reach_loop()
        .strictness(Strictness::Preferred)
        .validate(&empty, 10);
    assert!(soft.is_ok(), "a Preferred spine must not block generation");
    assert!(!soft.warnings.is_empty(), "but it must still be surfaced");
}

// ---------------------------------------------------------------------------------------------
// The host side: a guarantee you cannot find is not a guarantee
// ---------------------------------------------------------------------------------------------

#[test]
fn a_host_can_find_the_guaranteed_rooms_in_the_descriptor() {
    let (g, _) = world(2, 7);
    let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
    let template = per_reach_loop();
    let instances = SpineInstantiator::new(&g)
        .with_template(template.clone())
        .instantiate(&mut mission, &Rng::new(2));

    let mut b = DescriptorBuilder::new(&g, Fingerprint::from_raw(0x1234_5678), 42);
    for instance in &instances {
        for assignment in &instance.assignments {
            assert!(b.tag_spine_slot(
                assignment.scope,
                SpineSlotTag {
                    template: instance.template,
                    slot: assignment.slot.clone(),
                },
            ));
        }
    }
    let descriptor = b.finish();
    assert!(descriptor.check().is_none());

    // One tagged terminal per Reach, and it is a Space — the host asks by name, not by inference.
    let terminals: Vec<_> = descriptor
        .spine_slots(template.id, "terminal")
        .map(|(r, s)| (r, s.kind))
        .collect();
    assert_eq!(terminals.len(), 2);
    assert!(terminals.iter().all(|(_, kind)| *kind == NodeKind::Space));
    assert!(descriptor.spine_slot(template.id, "capstone").is_some());
    assert!(
        descriptor.spine_slot(template.id, "nonexistent").is_none(),
        "an unknown slot name must answer nothing rather than guess"
    );

    // And the tag survives the round-trip a host actually receives.
    let bytes = cv_core::serialize::to_bytes(&descriptor);
    let back: cv_core::WorldDescriptor = cv_core::serialize::from_bytes(&bytes).unwrap();
    assert_eq!(back, descriptor);
}

// ---------------------------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------------------------

#[test]
fn spining_is_deterministic_and_seed_sensitive() {
    let (g, _) = world(2, 8);
    let run = |seed: u64| {
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(dungeon())
            .with_template(per_reach_loop())
            .instantiate(&mut mission, &Rng::new(seed));
        (mission, instances)
    };
    assert_eq!(run(9), run(9), "same seed, same world");

    // Different seeds must be able to reach different grants, or the "generator picks" claim is
    // hollow — the slot would be a constant dressed up as a choice.
    let grants: Vec<Option<ObjectId>> = (0..32u64)
        .map(|s| {
            run(s)
                .1
                .iter()
                .find(|i| i.template == spine_id("dungeon"))
                .and_then(|i| i.granted_by("precursor"))
        })
        .collect();
    let distinct: std::collections::BTreeSet<_> = grants.iter().collect();
    assert!(
        distinct.len() > 1,
        "every seed picked the same grant; the choice is not a choice"
    );
}

#[test]
fn coverage_selects_the_same_instances_every_run() {
    let (g, _) = world(9, 5);
    let selected = |seed: u64| {
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        SpineInstantiator::new(&g)
            .with_template(per_reach_loop().coverage(Coverage::Every(3)))
            .instantiate(&mut mission, &Rng::new(seed))
            .iter()
            .map(|i| i.scope)
            .collect::<Vec<_>>()
    };
    let a = selected(1);
    assert_eq!(a.len(), 3, "reaches 0, 3, 6 of nine");
    assert_eq!(a, selected(2), "coverage is a pattern, not a per-seed roll");
    let _: BTreeMap<u32, u32> = BTreeMap::new();
}
