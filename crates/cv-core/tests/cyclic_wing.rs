//! The `cyclic-wing` scenario's interesting moment, both ways round.
//!
//! **Metroid Prime, Chozo Ruins.** Two ways out of the Main Plaza. One needs Missiles; the Missiles are
//! down the other. Geometry later finds a ledge in wing A reachable from wing B's balcony — an
//! **additive discovery** that would let a player into wing B's far side without ever opening the door.
//!
//! ⚠ **That is either a delightful sequence break or a broken gate, and the design has exactly one rule
//! for telling them apart.** This file runs that rule in both directions against the same ledge: the
//! door marked `GUARDED` refuses it, and the default `TOLERATED` adopts it. Nothing else about the
//! world changes between the two runs, which is what makes the per-lock policy the *cause*.
//!
//! ⚠ **Per-lock is the whole point.** Ten metres from the door, an optional missile-expansion alcove is
//! left `TOLERATED` on purpose, because breaking into *that* is a feature. A project-wide setting would
//! force one answer onto both.

use cv_core::adopt::{Adoption, AdoptionGate, Discovery, DiscoveryTrace, Refusal};
use cv_core::arena::Handle;
use cv_core::escalate::{AttemptBudget, EscalationReport, Failure, Layer, Response};
use cv_core::gate::SkipPolicy;
use cv_core::mission::{MissionEdge, MissionGraph, Rule};
use cv_core::node::{Node, NodeGraph};
use cv_core::object::ObjectId;
use cv_core::search::{Nudge, Target};
use cv_core::unlock::GrantMap;
use cv_core::verify::{Verification, Verifier};
use std::collections::{BTreeMap, BTreeSet};

fn missiles() -> ObjectId {
    ObjectId::derived("unlock", "Missiles")
}

/// The wings, and the edge index of the missile door.
struct World {
    mission: MissionGraph,
    wing_a: Handle<Node>,
    wing_b: Handle<Node>,
    door: usize,
}

fn world() -> World {
    let mut g = NodeGraph::new(1.0, 7);
    let area = g.add_child(g.root(), "chozo_ruins").unwrap();
    let plaza = g.add_child(area, "main_plaza").unwrap();
    let wing_a = g.add_child(area, "wing_a").unwrap();
    let wing_b = g.add_child(area, "wing_b").unwrap();
    let gallery = g.add_child(area, "ruined_gallery").unwrap();

    let mut mission = MissionGraph::new(plaza);
    // Fork and reconverge: the two wings diverge at the plaza and rejoin at the gallery.
    //
    // ⚠ **The rejoin is one-way, and it has to be.** A reversible reconvergence is itself a route
    // around the door — plaza → wing_a → gallery → wing_b — so the gate would verify as `Breached`
    // before geometry discovered anything at all. That is not a quirk of this fixture: **fork-and-
    // reconverge plus a two-way rejoin is a graph in which no gate on either fork can be exclusive**,
    // and a designer who marks one `GUARDED` there is asking for a proof the topology forbids. The
    // shape the scenario describes is the shape that works — you drop *into* the gallery from wing B,
    // and the loop is what makes the second visit cheap rather than what makes the gate skippable.
    mission.add_edge(MissionEdge::open(plaza, wing_a));
    mission.add_edge(MissionEdge::gated(plaza, wing_b, Rule::has(missiles())));
    mission.add_edge(MissionEdge::open(wing_a, gallery).one_way());
    mission.add_edge(MissionEdge::open(wing_b, gallery).one_way());

    World {
        mission,
        wing_a,
        wing_b,
        door: 1,
    }
}

/// The ledge geometry found: wing B's balcony reaches into wing A, with 0.4m of slack.
fn discovered_ledge(w: &World) -> Discovery {
    Discovery::Additive {
        edge: MissionEdge::open(w.wing_b, w.wing_a),
        slack: 0.4,
    }
}

/// What the verifier says about the door once the ledge is in the graph.
fn verify_with_ledge(w: &World) -> Verification {
    let mut with = w.mission.clone();
    with.add_edge(MissionEdge::open(w.wing_b, w.wing_a));
    // ⚠ The premise is the sphere the gate sits in, minus what the gate itself demands — so an empty
    // holding set here is sphere 0: `{ Walk, Jump }`, no Missiles.
    Verifier::new(&with)
        .verify(w.door, &BTreeSet::new(), &BTreeMap::new(), &GrantMap::new())
        .expect("the door is a gated edge")
}

#[test]
fn a_guarded_door_refuses_the_discovered_ledge() {
    let w = world();
    assert!(SkipPolicy::Guarded.requires_search());

    // The ledge does not open wing_b from wing_a directly — it goes the other way — so the sweep must
    // reason about the graph rather than the single edge.
    let verdict = verify_with_ledge(&w);

    let outcome = AdoptionGate::new()
        .guarding(w.door, verdict.clone())
        .decide(&discovered_ledge(&w));

    match &outcome {
        Adoption::Rejected {
            because: Refusal::GuardedGate { edge, .. },
        } => assert_eq!(*edge, w.door),
        other => panic!("a GUARDED gate must refuse a breach, got {other:?}"),
    }
    assert!(!outcome.adopted());
}

#[test]
fn the_same_ledge_is_adopted_when_the_policy_is_tolerated() {
    // ⚠ **Nothing about the world changes** — same graph, same ledge, same slack. `TOLERATED` runs no
    // sweep, so the gate has nothing to say and the edge lands.
    let w = world();
    assert!(!SkipPolicy::Tolerated.requires_search());

    let outcome = AdoptionGate::new().decide(&discovered_ledge(&w));
    assert_eq!(outcome, Adoption::Adopted { discovered: true });
    assert!(outcome.adopted());
}

#[test]
fn an_unproven_gate_refuses_as_firmly_as_a_breached_one() {
    // ⚠ *"Cannot prove absence"* is a loud failure, never a silent pass. Adopting on an unproven gate
    // would break the sacred gate quietly, which is the one outcome the policy exists to prevent.
    let w = world();
    let outcome = AdoptionGate::new()
        .guarding(w.door, Verification::Unproven { undecided: 1 })
        .decide(&discovered_ledge(&w));
    assert_eq!(
        outcome,
        Adoption::Rejected {
            because: Refusal::GuardedGate {
                edge: w.door,
                proven: false
            }
        }
    );
}

#[test]
fn the_rejection_escalates_to_geometry_and_closes_the_route() {
    let w = world();
    let outcome = AdoptionGate::new()
        .guarding(w.door, verify_with_ledge(&w))
        .decide(&discovered_ledge(&w));

    let (failure, response) = outcome.escalation(w.door).expect("a rejection escalates");
    assert_eq!(failure, Failure::Breached { edge: w.door });
    assert_eq!(
        response,
        Response::Escalated {
            to: Layer::Geometry
        },
        "only L4 can move the balcony that produced the ledge"
    );

    // ⚠ Closing the route is push-it-out with the sign flipped: the balcony moves until the margin is
    // definitely negative, which is the scenario's "balcony lowered 0.6m; margin now -0.2m".
    let nudge = Nudge::toward(0.4, Target::Closed, 0.2).expect("0.4 is not a closed margin");
    assert!(nudge.to < 0.0, "a closed route has a negative margin");
    assert!(nudge.resolved());
    assert!(nudge.distance() > 0.0);
}

#[test]
fn the_trace_says_what_happened_and_why() {
    let w = world();

    let rejected = DiscoveryTrace {
        from: w.wing_b,
        to: w.wing_a,
        slack: 0.4,
        outcome: AdoptionGate::new()
            .guarding(w.door, verify_with_ledge(&w))
            .decide(&discovered_ledge(&w)),
    };
    let line = rejected.to_string();
    assert!(line.contains("DISCOVERY"));
    assert!(line.contains("REJECTED"));
    assert!(
        line.contains("GUARDED"),
        "the trace must name the policy that refused it: {line}"
    );

    let adopted = DiscoveryTrace {
        from: w.wing_b,
        to: w.wing_a,
        slack: 0.4,
        outcome: AdoptionGate::new().decide(&discovered_ledge(&w)),
    };
    assert!(adopted
        .to_string()
        .contains("discovered rather than planned"));
}

#[test]
fn the_escalation_reaches_the_report_the_editor_renders() {
    // ⚠ The writer `M21 P03` has been missing: the view had a reader and no writer.
    let w = world();
    let mut report = EscalationReport::new();
    let mut budget = AttemptBudget::new(2);
    while !budget.exhausted() {
        budget.attempt();
    }
    report.record(budget.escalate(Failure::Breached { edge: w.door }, None));

    assert_eq!(report.len(), 1);
    assert!(
        report.drops().is_empty(),
        "a breach escalates to a layer that can fix it; nothing is dropped"
    );
    assert!(report.rows()[0].to_string().contains("escalated to L4"));
}

#[test]
fn the_gate_holds_when_no_ledge_was_ever_found() {
    // The control: without the discovery, the door verifies as exclusive, so `GUARDED` costs a sweep
    // and changes nothing — which is what it does in almost every world.
    let w = world();
    let verdict = Verifier::new(&w.mission)
        .verify(w.door, &BTreeSet::new(), &BTreeMap::new(), &GrantMap::new())
        .unwrap();
    assert_eq!(verdict, Verification::Exclusive);
    assert!(verdict.holds());
}

#[test]
fn a_two_way_reconvergence_makes_the_gate_unprovable_by_topology_alone() {
    // ⚠ **The control's control.** If the rejoin is reversible, the fork itself is the route around the
    // door — no discovery required. Worth a test of its own, because the failure looks identical to a
    // geometry bug from the trace alone, and it is not one: it is the graph the developer authored.
    let mut g = NodeGraph::new(1.0, 7);
    let area = g.add_child(g.root(), "chozo_ruins").unwrap();
    let plaza = g.add_child(area, "main_plaza").unwrap();
    let wing_a = g.add_child(area, "wing_a").unwrap();
    let wing_b = g.add_child(area, "wing_b").unwrap();
    let gallery = g.add_child(area, "ruined_gallery").unwrap();

    let mut mission = MissionGraph::new(plaza);
    mission.add_edge(MissionEdge::open(plaza, wing_a));
    mission.add_edge(MissionEdge::gated(plaza, wing_b, Rule::has(missiles())));
    mission.add_edge(MissionEdge::open(wing_a, gallery));
    mission.add_edge(MissionEdge::open(wing_b, gallery));

    let verdict = Verifier::new(&mission)
        .verify(1, &BTreeSet::new(), &BTreeMap::new(), &GrantMap::new())
        .unwrap();
    assert_eq!(
        verdict,
        Verification::Breached { via: wing_b },
        "a reversible rejoin is an alternative route, and GUARDED is right to say so"
    );
}
