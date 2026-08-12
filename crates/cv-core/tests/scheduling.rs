//! M08 exit criteria: a fixture pool and schedule produce a deterministic L1 plan, and
//! `AdaptiveRange` behaves the way the design says under every regime.
//!
//! The interesting assertions here are the ones about *degradation*. Anyone can make a scheduler hit
//! its target when content is plentiful; the design's claim is about what happens when it is not —
//! that a starved slot comes out **sparse rather than repetitive**, and says so.

use cv_core::serialize::to_bytes;
use cv_core::{
    AdaptiveRange, ContentKind, ContentPool, ContentRegistry, CountRule, Curve, NodeGraph,
    NodeKind, NodeState, Object, ObjectId, Progression, Schedule, ScheduleBook, Scheduler,
    SlotRule, Span, TargetOutcome, WorldLimit,
};
use cv_determinism::{Aabb, Rng, Vec3};

/// A world of `reaches` Reaches, each with one Area and two Spaces.
fn world(reaches: usize) -> NodeGraph {
    let mut g = NodeGraph::new(1.0, 1);
    for r in 0..reaches {
        let reach = g.add_child(g.root(), format!("reach_{r}")).unwrap();
        let area = g.add_child(reach, format!("area_{r}")).unwrap();
        for s in 0..2 {
            g.add_child(area, format!("space_{r}_{s}")).unwrap();
        }
    }
    for h in g.walk() {
        g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
            .unwrap();
    }
    for h in g.walk() {
        g.advance(h, NodeState::Realized).unwrap();
    }
    g
}

/// Content spanning the world: some early, some late, some throughout — plus a non-schedulable
/// `Component`, which must never reach the pool.
fn content() -> (ContentRegistry, ScheduleBook) {
    let mut reg = ContentRegistry::new();
    let mut book = ScheduleBook::new();

    let early = reg.register(ContentKind::Actor, "rubble", 1).unwrap();
    let late = reg.register(ContentKind::Actor, "sentinel", 2).unwrap();
    let always = reg.register(ContentKind::Actor, "door", 3).unwrap();
    let ramping = reg.register(ContentKind::Item, "relic", 4).unwrap();
    reg.register(ContentKind::Component, "hinge", 5).unwrap();

    book.set(early, Schedule::during(Span::until(0.4)));
    book.set(late, Schedule::during(Span::from(0.6)));
    book.set(always, Schedule::always());
    // Rare early, common late — the classic difficulty ramp.
    book.set(ramping, Schedule::always().weighted(Curve::ramp(0.1, 1.0)));

    (reg, book)
}

#[test]
fn l0_resolves_only_schedulable_content() {
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    // Five registered, but the Component is not something the algorithm places.
    assert_eq!(reg.len(), 5);
    assert_eq!(pool.len(), 4);
    assert!(
        !pool
            .entries()
            .iter()
            .any(|e| e.kind == ContentKind::Component),
        "a Component is referenced and composed, never placed on its own"
    );
    assert_eq!(pool.of_kind(ContentKind::Actor).count(), 3);
    assert_eq!(pool.of_kind(ContentKind::Item).count(), 1);
}

#[test]
fn eligibility_follows_progression() {
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);

    let at_start: Vec<ObjectId> = pool
        .eligible_at(Progression::START)
        .iter()
        .map(|c| c.content)
        .collect();
    let at_end: Vec<ObjectId> = pool
        .eligible_at(Progression::END)
        .iter()
        .map(|c| c.content)
        .collect();

    let rubble = ContentRegistry::id_for(ContentKind::Actor, "rubble");
    let sentinel = ContentRegistry::id_for(ContentKind::Actor, "sentinel");
    assert!(at_start.contains(&rubble) && !at_start.contains(&sentinel));
    assert!(at_end.contains(&sentinel) && !at_end.contains(&rubble));

    // The ramping item is eligible throughout, but weighted very differently at each end.
    let relic = ContentRegistry::id_for(ContentKind::Item, "relic");
    let w_start = pool
        .eligible_at(Progression::START)
        .iter()
        .find(|c| c.content == relic)
        .unwrap()
        .weight;
    let w_end = pool
        .eligible_at(Progression::END)
        .iter()
        .find(|c| c.content == relic)
        .unwrap()
        .weight;
    assert!(
        w_end > w_start * 5.0,
        "the ramp should strongly favour the late world"
    );
}

#[test]
fn the_plan_is_deterministic() {
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    let g = world(4);

    let build = || {
        Scheduler::new(&g, &pool)
            .with_rule(SlotRule::new(
                NodeKind::Space,
                AdaptiveRange::new(1, 4).with_jitter(1),
            ))
            .plan(&Rng::new(0xBEEF))
    };
    let a = build();
    let b = build();
    assert_eq!(a, b);
    assert_eq!(a.len(), 8, "one slot per Space");

    // A different seed explores a different plan, but only via jitter.
    let other = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(
            NodeKind::Space,
            AdaptiveRange::new(1, 4).with_jitter(1),
        ))
        .plan(&Rng::new(0xF00D));
    assert_ne!(
        a.total_target(),
        other.total_target(),
        "jitter should differ across seeds"
    );
}

#[test]
fn only_scopes_with_a_rule_are_planned() {
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    let g = world(3);

    let spaces_only = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, AdaptiveRange::new(0, 3)))
        .plan(&Rng::new(1));
    assert_eq!(spaces_only.len(), 6);

    // Adding an Area rule plans those too, without disturbing the Space slots.
    let both = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, AdaptiveRange::new(0, 3)))
        .with_rule(SlotRule::new(NodeKind::Area, AdaptiveRange::new(0, 1)))
        .plan(&Rng::new(1));
    assert_eq!(both.len(), 9, "6 Spaces + 3 Areas");
    for slot in spaces_only.slots() {
        assert_eq!(both.slot(slot.scope).unwrap().target, slot.target);
    }
}

#[test]
fn a_starved_world_reads_sparse_rather_than_repetitive() {
    // One piece of content, a ceiling of six. A naive scheduler would place six copies of the same
    // thing; the adaptive one places one and *reports* that it is scarce.
    let mut reg = ContentRegistry::new();
    reg.register(ContentKind::Actor, "only_thing", 1).unwrap();
    let pool = ContentPool::resolve(&reg, &ScheduleBook::new());
    let g = world(2);

    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(
            NodeKind::Space,
            AdaptiveRange::new(4, 6).with_repeat_tol(1.5),
        ))
        .plan(&Rng::new(1));

    for slot in plan.slots() {
        assert_eq!(slot.reasoning.outcome, TargetOutcome::Scarce);
        assert_eq!(
            slot.target, 1,
            "floor(1 × 1.5 × 1.0) = 1, and it is not padded up to soft_min"
        );
        assert!(slot.target < 4, "soft_min is a preference, not a floor");
    }
    assert_eq!(
        plan.scarce_slots().count(),
        plan.len(),
        "every slot is starved, and every one says so"
    );
}

#[test]
fn an_abundant_world_stops_at_the_ceiling() {
    let mut reg = ContentRegistry::new();
    for i in 0..40 {
        reg.register(ContentKind::Actor, format!("thing_{i}"), i as u64)
            .unwrap();
    }
    let pool = ContentPool::resolve(&reg, &ScheduleBook::new());
    let g = world(2);

    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, AdaptiveRange::new(2, 5)))
        .plan(&Rng::new(1));

    for slot in plan.slots() {
        assert_eq!(slot.reasoning.outcome, TargetOutcome::Abundant);
        assert_eq!(slot.target, 5, "hard_max is a true ceiling");
        assert_eq!(
            slot.candidates.len(),
            40,
            "all of it remains available to draw from"
        );
    }
}

#[test]
fn jitter_is_keyed_on_identity_so_inserting_a_scope_does_not_reshuffle_the_rest() {
    // The property the scheduler's docs claim: a slot's wobble is forked on the scope's *identity*,
    // not its position, so adding a room early does not silently re-roll every later room.
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    let rule = SlotRule::new(NodeKind::Space, AdaptiveRange::new(1, 5).with_jitter(2));
    let rng = Rng::new(0xABCD);

    let mut g = world(2);
    let before: Vec<(String, u32)> = Scheduler::new(&g, &pool)
        .with_rule(rule.clone())
        .plan(&rng)
        .slots()
        .iter()
        .map(|s| (g.node(s.scope).unwrap().name().to_string(), s.target))
        .collect();

    // Insert a new Space into the *first* Area, shifting everything after it.
    let first_area = g.of_kind(NodeKind::Area).next().unwrap().0;
    let inserted = g.add_child(first_area, "space_inserted").unwrap();
    g.set_envelope(inserted, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
        .unwrap();
    g.advance(inserted, NodeState::Realized).unwrap();

    let after: Vec<(String, u32)> = Scheduler::new(&g, &pool)
        .with_rule(rule)
        .plan(&rng)
        .slots()
        .iter()
        .map(|s| (g.node(s.scope).unwrap().name().to_string(), s.target))
        .collect();

    for (name, target) in &before {
        let found = after
            .iter()
            .find(|(n, _)| n == name)
            .expect("original spaces survive");
        assert_eq!(
            found.1, *target,
            "space {name} was re-rolled by an unrelated insertion — jitter is position-keyed"
        );
    }
    assert_eq!(after.len(), before.len() + 1);
}

#[test]
fn progression_spreads_across_the_world() {
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    let g = world(5);
    let scheduler = Scheduler::new(&g, &pool);

    let firsts: Vec<f64> = g
        .of_kind(NodeKind::Reach)
        .map(|(h, _)| scheduler.progression_of(h).value())
        .collect();
    assert_eq!(firsts, vec![0.0, 0.25, 0.5, 0.75, 1.0]);

    // A one-Reach world sits at the start rather than dividing by zero.
    let tiny = world(1);
    let tiny_pool = ContentPool::resolve(&reg, &book);
    let tiny_sched = Scheduler::new(&tiny, &tiny_pool);
    let reach = tiny.of_kind(NodeKind::Reach).next().unwrap().0;
    assert_eq!(tiny_sched.progression_of(reach), Progression::START);
}

#[test]
fn early_and_late_slots_draw_from_different_content() {
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    let g = world(4);
    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, AdaptiveRange::new(1, 4)))
        .plan(&Rng::new(1));

    let rubble = ContentRegistry::id_for(ContentKind::Actor, "rubble");
    let sentinel = ContentRegistry::id_for(ContentKind::Actor, "sentinel");

    let first = &plan.slots()[0];
    let last = plan.slots().last().unwrap();
    assert!(first.candidates.iter().any(|c| c.content == rubble));
    assert!(!first.candidates.iter().any(|c| c.content == sentinel));
    assert!(last.candidates.iter().any(|c| c.content == sentinel));
    assert!(!last.candidates.iter().any(|c| c.content == rubble));
}

#[test]
fn an_empty_pool_plans_empty_slots_rather_than_failing() {
    let g = world(2);
    let pool = ContentPool::resolve(&ContentRegistry::new(), &ScheduleBook::new());
    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, AdaptiveRange::new(2, 5)))
        .plan(&Rng::new(1));
    assert_eq!(plan.len(), 4);
    assert_eq!(
        plan.total_target(),
        0,
        "nothing to place is a valid plan, not an error"
    );
    for slot in plan.slots() {
        assert!(slot.candidates.is_empty());
        assert_eq!(slot.reasoning.outcome, TargetOutcome::Scarce);
    }
}

#[test]
fn the_plan_explains_every_target() {
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    let g = world(3);
    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(
            NodeKind::Space,
            AdaptiveRange::new(2, 4).with_repeat_tol(1.5),
        ))
        .plan(&Rng::new(1));

    for slot in plan.slots() {
        let r = &slot.reasoning;
        // The derivation is reproducible from the recorded inputs.
        let expected = (r.unique as f64 * r.repeat_tol * r.weight).floor() as u32;
        assert_eq!(
            r.supported, expected,
            "reasoning must match the formula it reports"
        );
        assert_eq!(r.target, slot.target);
        assert!(r.to_string().contains("unique"));
    }
}

// ---------------------------------------------------------------------------------------------
// Opt-in dev control: the alternatives to adaptive counting
// ---------------------------------------------------------------------------------------------

#[test]
fn a_fixed_count_ignores_how_much_content_exists() {
    // A Portal-style chamber holds exactly one puzzle whether the library has two pieces or two
    // hundred. Adaptation is the default, not a mandate.
    let g = world(2);
    let sparse = {
        let mut r = ContentRegistry::new();
        r.register(ContentKind::Actor, "only", 1).unwrap();
        ContentPool::resolve(&r, &ScheduleBook::new())
    };
    let rich = {
        let mut r = ContentRegistry::new();
        for i in 0..50 {
            r.register(ContentKind::Actor, format!("thing_{i}"), i as u64)
                .unwrap();
        }
        ContentPool::resolve(&r, &ScheduleBook::new())
    };

    for pool in [&sparse, &rich] {
        let plan = Scheduler::new(&g, pool)
            .with_rule(SlotRule::new(NodeKind::Space, CountRule::fixed(1)))
            .plan(&Rng::new(1));
        for slot in plan.slots() {
            assert_eq!(slot.target, 1);
            assert_eq!(slot.reasoning.outcome, TargetOutcome::Fixed);
            assert!(
                !slot.reasoning.outcome.is_adaptive(),
                "the dev took control here"
            );
        }
    }
    // A fixed slot is never reported as scarce — that would bury the real signal.
    let plan = Scheduler::new(&g, &sparse)
        .with_rule(SlotRule::new(NodeKind::Space, CountRule::fixed(1)))
        .plan(&Rng::new(1));
    assert_eq!(plan.scarce_slots().count(), 0);
}

#[test]
fn a_plain_range_varies_without_adapting() {
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    let g = world(6);

    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, CountRule::range(2, 5)))
        .plan(&Rng::new(0xABC));

    let targets: Vec<u32> = plan.slots().iter().map(|s| s.target).collect();
    assert!(
        targets.iter().all(|t| (2..=5).contains(t)),
        "always inside the range: {targets:?}"
    );
    assert!(targets.iter().any(|t| *t != targets[0]), "and it does vary");
    for slot in plan.slots() {
        assert_eq!(slot.reasoning.outcome, TargetOutcome::Sampled);
    }
    // Deterministic across runs.
    let again = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, CountRule::range(2, 5)))
        .plan(&Rng::new(0xABC));
    assert_eq!(again, plan);
}

#[test]
fn a_curve_drives_density_over_progression() {
    // "Sparse at the entrance, busy in the depths" — without touching variety at all.
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    let g = world(5);

    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(
            NodeKind::Space,
            CountRule::curve(Curve::ramp(1.0, 6.0)),
        ))
        .plan(&Rng::new(1));

    let first = plan.slots().first().unwrap();
    let last = plan.slots().last().unwrap();
    assert_eq!(first.target, 1, "start of the ramp");
    assert_eq!(last.target, 6, "end of the ramp");
    assert_eq!(first.reasoning.outcome, TargetOutcome::Curved);
    // Monotonically non-decreasing along the world.
    let targets: Vec<u32> = plan.slots().iter().map(|s| s.target).collect();
    assert!(targets.windows(2).all(|w| w[0] <= w[1]), "{targets:?}");
}

#[test]
fn chance_gates_whether_content_is_offered_at_all() {
    // Weight and chance are different: this piece is *always* strongly preferred when present, but
    // only present a fraction of the time.
    let mut reg = ContentRegistry::new();
    let common = reg.register(ContentKind::Actor, "common", 1).unwrap();
    let rare = reg.register(ContentKind::Actor, "rare", 2).unwrap();
    let mut book = ScheduleBook::new();
    book.set(common, Schedule::always());
    book.set(rare, Schedule::always().with_chance(0.25));
    let pool = ContentPool::resolve(&reg, &book);

    let g = world(20);
    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, CountRule::fixed(1)))
        .plan(&Rng::new(0xC0FFEE));

    let total = plan.len();
    let with_rare = plan
        .slots()
        .iter()
        .filter(|s| s.candidates.iter().any(|c| c.content == rare))
        .count();
    let with_common = plan
        .slots()
        .iter()
        .filter(|s| s.candidates.iter().any(|c| c.content == common))
        .count();

    assert_eq!(with_common, total, "chance 1.0 is always offered");
    assert!(
        with_rare > 0 && with_rare < total,
        "chance 0.25 appears sometimes: {with_rare}/{total}"
    );
    // Roughly a quarter, with slack for a 40-slot sample.
    let ratio = with_rare as f64 / total as f64;
    assert!((0.05..0.55).contains(&ratio), "expected ~0.25, got {ratio}");

    // And it is reproducible.
    let again = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, CountRule::fixed(1)))
        .plan(&Rng::new(0xC0FFEE));
    assert_eq!(again, plan);
}

#[test]
fn content_is_only_offered_to_scopes_that_can_hold_it() {
    // The correctness point: a Biome dresses an Area and can never go in a room. Counting it among a
    // room's variety would inflate that room's adaptive target with content it could never use.
    let mut reg = ContentRegistry::new();
    let actor = reg.register(ContentKind::Actor, "prop", 1).unwrap();
    let biome = reg.register(ContentKind::Biome, "caverns", 2).unwrap();
    let mut book = ScheduleBook::new();
    book.set(actor, Schedule::for_kind(ContentKind::Actor));
    book.set(biome, Schedule::for_kind(ContentKind::Biome));
    let pool = ContentPool::resolve(&reg, &book);

    let g = world(2);
    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, AdaptiveRange::new(0, 5)))
        .with_rule(SlotRule::new(NodeKind::Area, AdaptiveRange::new(0, 5)))
        .plan(&Rng::new(1));

    for slot in plan.slots() {
        let kind = g.node(slot.scope).unwrap().kind();
        let offered: Vec<ObjectId> = slot.candidates.iter().map(|c| c.content).collect();
        match kind {
            NodeKind::Space => {
                assert!(offered.contains(&actor));
                assert!(!offered.contains(&biome), "a Biome cannot go in a room");
                assert_eq!(
                    slot.reasoning.unique, 1,
                    "unique must not count unusable content"
                );
            }
            NodeKind::Area => {
                assert!(offered.contains(&biome));
                assert!(!offered.contains(&actor), "an Actor is not Area dressing");
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn scopes_can_be_overridden_per_content() {
    // A dev who wants an Actor placed at Area scope just says so.
    let mut reg = ContentRegistry::new();
    let landmark = reg.register(ContentKind::Actor, "landmark", 1).unwrap();
    let mut book = ScheduleBook::new();
    book.set(landmark, Schedule::always().in_scopes([NodeKind::Area]));
    let pool = ContentPool::resolve(&reg, &book);

    let g = world(2);
    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, AdaptiveRange::new(0, 3)))
        .with_rule(SlotRule::new(NodeKind::Area, AdaptiveRange::new(0, 3)))
        .plan(&Rng::new(1));

    for slot in plan.slots() {
        let offered = slot.candidates.iter().any(|c| c.content == landmark);
        assert_eq!(
            offered,
            g.node(slot.scope).unwrap().kind() == NodeKind::Area
        );
    }
}

#[test]
fn world_limits_express_what_per_slot_counts_cannot() {
    // "Exactly one final boss" is a world-wide fact. No per-slot count can say it.
    let mut reg = ContentRegistry::new();
    let boss = reg.register(ContentKind::Actor, "final_boss", 1).unwrap();
    let save = reg.register(ContentKind::Actor, "save_room", 2).unwrap();
    let filler = reg.register(ContentKind::Actor, "rubble", 3).unwrap();
    let mut book = ScheduleBook::new();
    book.set(
        boss,
        Schedule::during(Span::from(0.8)).limited(WorldLimit::exactly(1)),
    );
    book.set(save, Schedule::always().limited(WorldLimit::at_most(3)));
    book.set(filler, Schedule::always());
    let pool = ContentPool::resolve(&reg, &book);

    // Only the boss is *required*; a cap alone is not a demand.
    let demands: Vec<ObjectId> = pool.demands().map(|(id, _)| id).collect();
    assert_eq!(demands, vec![boss]);
    assert!(pool.world_limit(boss).is_required());
    assert!(!pool.world_limit(save).is_required());
    assert_eq!(pool.world_limit(filler), WorldLimit::UNLIMITED);

    // Limits are checkable.
    assert!(WorldLimit::exactly(1).permits(1));
    assert!(!WorldLimit::exactly(1).permits(2));
    assert!(!WorldLimit::exactly(1).permits(0));
    assert!(
        WorldLimit::at_most(3).permits(0),
        "a cap does not require any"
    );

    // And the plan carries the demand through to L2, which is what actually places.
    let g = world(3);
    let plan = Scheduler::new(&g, &pool)
        .with_rule(SlotRule::new(NodeKind::Space, AdaptiveRange::new(0, 4)))
        .plan(&Rng::new(1));
    assert_eq!(plan.demands().len(), 1);
    assert_eq!(plan.demands()[0].0, boss);
    assert_eq!(plan.demands()[0].1, WorldLimit::exactly(1));
}

#[test]
fn count_rules_all_report_their_reasoning() {
    let (reg, book) = content();
    let pool = ContentPool::resolve(&reg, &book);
    let g = world(3);

    for rule in [
        CountRule::fixed(2),
        CountRule::range(1, 4),
        CountRule::curve(Curve::constant(3.0)),
        CountRule::adaptive(AdaptiveRange::new(1, 4)),
    ] {
        let plan = Scheduler::new(&g, &pool)
            .with_rule(SlotRule::new(NodeKind::Space, rule.clone()))
            .plan(&Rng::new(5));
        for slot in plan.slots() {
            assert_eq!(slot.reasoning.target, slot.target);
            // Even a non-adaptive rule records what variety was available, so a dev switching to
            // Adaptive can see what it would have chosen.
            assert_eq!(slot.reasoning.unique, slot.candidates.len() as u32);
            assert!(!slot.reasoning.to_string().is_empty());
        }
    }
}

#[test]
fn schedules_survive_the_project_round_trip() {
    // Schedules are project config, so they are a fingerprint input and must serialize stably.
    let (_, book) = content();
    assert_eq!(to_bytes(&book), to_bytes(&book));
    let restored: ScheduleBook = cv_core::serialize::from_bytes(&to_bytes(&book)).unwrap();
    assert_eq!(restored, book);
}
