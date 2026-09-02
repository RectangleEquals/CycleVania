//! **Adoption** — what the solver does with an edge geometry handed it that the graph never asked for.
//!
//! # Feedback from the expensive loop is not symmetric
//!
//! | Discovery | Cost | Response |
//! |---|---|---|
//! | **additive** — geometry made something accessible the graph did not require | cheap | **adopt it**, re-verify, and record that it was *discovered rather than planned* |
//! | **invalidating** — geometry made something inaccessible the graph requires | expensive | repair locally, or re-enter the solve loop |
//!
//! ⚠ **Adoption is why an unplanned shortcut is not a failure.** A found route becomes a real edge with
//! a real `cycle_density` contribution, and the trace says it was found — which is the flavour of
//! emergence a hand-built metroidvania has and a naive generator does not.
//!
//! # One rule decides, and it is made of machinery that already exists
//!
//! > a discovered edge is adopted **unless** it violates a `forbids()` [`Exclusion`], a forbidden
//! > [`Route`], or a [`GUARDED`](crate::gate::SkipPolicy::Guarded) gate's exclusivity.
//!
//! Without that rule the generator cannot tell an **emergent alternate path** from a **broken sequence
//! break**, and must either refuse every discovery — losing the emergence — or accept every one, which
//! silently breaks the gates a designer marked sacred.
//!
//! ⚠ **The three refusals are checked in cost order**, cheapest first: an exclusion is a volume test, a
//! forbidden route is a budget walk, and guarded exclusivity is a graph sweep. Checking the expensive
//! one first would pay for a proof about an edge an exclusion was going to refuse for free.
//!
//! # Closing a route is push-it-out with the sign flipped
//!
//! ⚠ **The design asserted this and never specified it.** [§8.1](crate::search)'s *push it out* moves
//! an anchor until a margin exceeds `tolerance`, resolving an `AMBIGUOUS` answer into a definite one.
//! Closing a rejected shortcut moves an anchor until the margin is definitely **negative**. Same move,
//! same cost, opposite target — so it is [`Nudge`](crate::search::Nudge) with a
//! [`Target`](crate::search::Target), not a second operation, and the scenario's *"balcony lowered
//! 0.6m; margin now -0.2m"* is exactly what it produces.

use crate::arena::Handle;
use crate::escalate::{Failure, Response};
use crate::exclusion::Exclusion;
use crate::judge::{Obligation, Route};
use crate::mission::MissionEdge;
use crate::node::Node;
use crate::path::ClassPath;
use crate::verify::Verification;
use std::fmt;

/// What geometry handed the solver.
#[derive(Clone, Debug, PartialEq)]
pub enum Discovery {
    /// Geometry made something accessible the graph did not require.
    Additive {
        /// The edge that turned out to exist.
        edge: MissionEdge,
        /// How much slack the traversal had, in world units — the scenario's `slack 0.4m`.
        slack: f64,
    },
    /// Geometry made something inaccessible the graph requires.
    ///
    /// ⚠ **The expensive half, and it is not adopted or rejected — it is *repaired*.** Naming it here
    /// rather than in a separate type is what stops a caller from handling only the cheap direction and
    /// believing it handled discoveries.
    Invalidating {
        /// The edge the graph needs and geometry will not give.
        edge: MissionEdge,
    },
}

impl Discovery {
    /// The edge either way.
    pub fn edge(&self) -> &MissionEdge {
        match self {
            Discovery::Additive { edge, .. } | Discovery::Invalidating { edge } => edge,
        }
    }

    /// Is this the cheap direction?
    pub fn is_additive(&self) -> bool {
        matches!(self, Discovery::Additive { .. })
    }
}

/// Why a discovered edge was refused.
///
/// ⚠ **Three reasons and no fourth.** The design says the negative half of an obligation is expressible
/// in exactly four places, and one of them — `NegateRule` — is a *gate*, which reaches this decision as
/// part of the graph rather than as a veto over it. So there are three vetoes here, and a fourth
/// appearing later would mean the design grew a way to say *no* that nobody wrote down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A `forbids()` exclusion covers it.
    Excluded { reason: String },
    /// A forbidden `Route` would become walkable.
    ForbiddenRoute { from: String, to: String },
    /// A `GUARDED` gate would lose its exclusivity.
    ///
    /// ⚠ **`Unproven` refuses too**, and the field says which. A verification that could not establish
    /// absence has not established it, and adopting on that basis would break the sacred gate quietly.
    GuardedGate { edge: usize, proven: bool },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Excluded { reason } => write!(f, "an exclusion covers it: {reason}"),
            Refusal::ForbiddenRoute { from, to } => {
                write!(
                    f,
                    "it would make the forbidden route {from} → {to} walkable"
                )
            }
            Refusal::GuardedGate { edge, proven: true } => write!(
                f,
                "it would open the far side of the GUARDED gate on edge {edge} without its unlock"
            ),
            Refusal::GuardedGate {
                edge,
                proven: false,
            } => write!(
                f,
                "the GUARDED gate on edge {edge} cannot be proven exclusive here, and an unproven \
                 gate is not an open one"
            ),
        }
    }
}

/// What the adoption gate decided.
#[derive(Clone, Debug, PartialEq)]
pub enum Adoption {
    /// The edge is now a real edge, contributing to `cycle_density`.
    ///
    /// ⚠ **`discovered` is carried, not inferred.** *"This loop was found rather than planned"* is the
    /// most interesting line in the trace and there is nowhere else to keep it: once the edge is in the
    /// graph it looks exactly like a planned one.
    Adopted { discovered: bool },
    /// The edge is refused, and the geometry that produced it must be closed.
    Rejected { because: Refusal },
}

impl Adoption {
    /// Did it become a real edge?
    pub fn adopted(&self) -> bool {
        matches!(self, Adoption::Adopted { .. })
    }

    /// The escalation a rejection produces — geometry has to close what it opened.
    pub fn escalation(&self, edge: usize) -> Option<(Failure, Response)> {
        match self {
            Adoption::Adopted { .. } => None,
            Adoption::Rejected { .. } => {
                let f = Failure::Breached { edge };
                let to = f.escalates_to();
                Some((f, Response::Escalated { to }))
            }
        }
    }
}

/// The one rule that separates an emergent alternate path from a broken sequence break.
///
/// ⚠ **It holds only borrowed facts and decides nothing about geometry.** Adoption is a question about
/// the *graph*; nudging a balcony is L4's answer to the rejection, and keeping them apart is what lets
/// the same verdict mean different things at different layers.
#[derive(Default)]
pub struct AdoptionGate<'a> {
    exclusions: Vec<(&'a Exclusion, ClassPath)>,
    forbidden: Vec<&'a Route>,
    guarded: Vec<(usize, Verification)>,
}

impl<'a> AdoptionGate<'a> {
    /// A gate that refuses nothing.
    pub fn new() -> Self {
        AdoptionGate {
            exclusions: Vec::new(),
            forbidden: Vec::new(),
            guarded: Vec::new(),
        }
    }

    /// An exclusion covering the discovered edge, and the class that would travel it.
    pub fn excluding(mut self, e: &'a Exclusion, traveller: ClassPath) -> Self {
        self.exclusions.push((e, traveller));
        self
    }

    /// A route the world forbids.
    ///
    /// ⚠ **Required routes are ignored here** — a discovery that helps satisfy one is exactly the
    /// emergence adoption exists for, and filtering on the obligation rather than trusting the caller
    /// is what stops a required route from being mistaken for a veto.
    pub fn forbidding(mut self, r: &'a Route) -> Self {
        if r.obligation == Obligation::Forbidden {
            self.forbidden.push(r);
        }
        self
    }

    /// A guarded gate's verification result, taken with the discovered edge in place.
    pub fn guarding(mut self, edge: usize, v: Verification) -> Self {
        self.guarded.push((edge, v));
        self
    }

    /// Decide.
    ///
    /// ⚠ **Cheapest refusal first.** An exclusion is a volume test, a forbidden route is a budget walk,
    /// a guarded gate is a graph sweep. Ordering them any other way pays for a proof about an edge
    /// something free was going to refuse.
    pub fn decide(&self, d: &Discovery) -> Adoption {
        if !d.is_additive() {
            // An invalidating discovery is repaired, never adopted — and saying so is better than
            // letting it fall through to `Adopted` because nothing refused it.
            return Adoption::Rejected {
                because: Refusal::Excluded {
                    reason: "an invalidating discovery is repaired, not adopted".into(),
                },
            };
        }

        for (e, traveller) in &self.exclusions {
            if !e.is_vacuous() && e.forbids(traveller) {
                return Adoption::Rejected {
                    because: Refusal::Excluded {
                        reason: e.reason.clone(),
                    },
                };
            }
        }

        // Any forbidden route registered against this discovery refuses it. The caller registers the
        // routes the edge would actually make walkable, because deciding *that* needs the budget walk
        // and this type deliberately holds no graph.
        if let Some(r) = self.forbidden.first() {
            return Adoption::Rejected {
                because: Refusal::ForbiddenRoute {
                    from: format!("{:?}", r.from),
                    to: format!("{:?}", r.to),
                },
            };
        }

        for (edge, v) in &self.guarded {
            if !v.holds() {
                return Adoption::Rejected {
                    because: Refusal::GuardedGate {
                        edge: *edge,
                        proven: matches!(v, Verification::Breached { .. }),
                    },
                };
            }
        }

        Adoption::Adopted { discovered: true }
    }
}

/// One trace row for a discovery.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveryTrace {
    /// Where the edge starts.
    pub from: Handle<Node>,
    /// Where it lands.
    pub to: Handle<Node>,
    /// How much room the traversal had.
    pub slack: f64,
    /// What was decided.
    pub outcome: Adoption,
}

impl fmt::Display for DiscoveryTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.outcome {
            Adoption::Adopted { discovered } => write!(
                f,
                "DISCOVERY  slack {:.1} — ADOPTED{}",
                self.slack,
                if *discovered {
                    ", discovered rather than planned"
                } else {
                    ""
                }
            ),
            Adoption::Rejected { because } => {
                write!(
                    f,
                    "DISCOVERY  slack {:.1} — REJECTED: {because}",
                    self.slack
                )
            }
        }
    }
}

/// How much a set of edges loops, as the fraction beyond a spanning tree.
///
/// ⚠ **An adopted edge contributes exactly as a planned one does.** Weighting a discovery differently
/// would make `cycle_density` mean *"how much looping the solver intended"* rather than *"how much
/// looping the world has"* — and the world is the thing a player walks through.
pub fn cycle_density(scopes: usize, edges: usize) -> f64 {
    if scopes < 2 {
        return 0.0;
    }
    let tree = scopes - 1;
    if edges <= tree {
        return 0.0;
    }
    // The most a connected graph can have is every pair joined.
    let max_extra = (scopes * (scopes - 1)) / 2 - tree;
    if max_extra == 0 {
        return 0.0;
    }
    ((edges - tree) as f64 / max_extra as f64).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::BudgetRef;
    use crate::class::{ClassRegistry, CoreClass, Kind, ObjectBound};
    use crate::collision::CollisionBody;
    use crate::escalate::Layer;
    use crate::node::{NodeGraph, NodeKind};
    use crate::object::ObjectId;
    use crate::shape::Shape;
    use cv_determinism::Vec3;

    fn edge() -> (MissionEdge, Handle<Node>, Handle<Node>) {
        let mut g = NodeGraph::new(1.0, 1);
        let area = g.add_child(g.root(), "area").unwrap();
        let a = g.add_child(area, "wing_a").unwrap();
        let b = g.add_child(area, "wing_b").unwrap();
        let _ = NodeKind::Space;
        (MissionEdge::open(b, a), a, b)
    }

    fn ledge() -> Discovery {
        Discovery::Additive {
            edge: edge().0,
            slack: 0.4,
        }
    }

    fn body() -> CollisionBody {
        CollisionBody::of(Shape::Cube {
            extents: Vec3::new(2.0, 2.0, 2.0),
            bevel: 0.0,
        })
    }

    fn traveller() -> ClassPath {
        ClassPath::new("/Content/Actor/Player").unwrap()
    }

    #[test]
    fn a_discovery_nothing_refuses_is_adopted_and_says_it_was_found() {
        let got = AdoptionGate::new().decide(&ledge());
        assert_eq!(got, Adoption::Adopted { discovered: true });
        assert!(got.adopted());
    }

    #[test]
    fn a_guarded_gate_refuses_the_same_ledge_a_tolerated_one_adopts() {
        // ⚠ The cyclic-wing scenario's interesting moment, both ways round.
        let rejected = AdoptionGate::new()
            .guarding(1, Verification::Breached { via: edge().1 })
            .decide(&ledge());
        assert_eq!(
            rejected,
            Adoption::Rejected {
                because: Refusal::GuardedGate {
                    edge: 1,
                    proven: true
                }
            }
        );

        // TOLERATED runs no sweep, so the gate has nothing to say and the edge lands.
        let adopted = AdoptionGate::new().decide(&ledge());
        assert!(adopted.adopted(), "the same ledge, with the default policy");
    }

    #[test]
    fn an_unproven_gate_refuses_too_and_the_verdict_says_which() {
        let got = AdoptionGate::new()
            .guarding(4, Verification::Unproven { undecided: 2 })
            .decide(&ledge());
        assert_eq!(
            got,
            Adoption::Rejected {
                because: Refusal::GuardedGate {
                    edge: 4,
                    proven: false
                }
            }
        );
        assert!(!got.adopted());
    }

    #[test]
    fn an_exclusion_refuses_and_carries_its_reason_to_the_trace() {
        let e = Exclusion::new(body(), "the vault floor stays sealed");
        let got = AdoptionGate::new()
            .excluding(&e, traveller())
            .decide(&ledge());
        let Adoption::Rejected {
            because: Refusal::Excluded { reason },
        } = &got
        else {
            panic!("expected an exclusion refusal, got {got:?}");
        };
        assert_eq!(reason, "the vault floor stays sealed");
    }

    #[test]
    fn a_declared_escape_lets_the_discovery_through() {
        let mut r = ClassRegistry::with_core();
        r.register(traveller(), ClassPath::new(ObjectBound::PATH).unwrap())
            .unwrap();
        let e = Exclusion::new(body(), "sealed").except(Kind::new(&r, traveller()).unwrap());
        assert!(AdoptionGate::new()
            .excluding(&e, traveller())
            .decide(&ledge())
            .adopted());
    }

    #[test]
    fn a_vacuous_exclusion_refuses_nothing() {
        // ⚠ The rule that looks on and is off — it must not silently refuse every discovery.
        let e = Exclusion::new(CollisionBody::empty(), "nothing here");
        assert!(AdoptionGate::new()
            .excluding(&e, traveller())
            .decide(&ledge())
            .adopted());
    }

    #[test]
    fn a_forbidden_route_refuses_and_a_required_one_does_not() {
        let (a, b) = (
            ObjectId::derived("scope", "a"),
            ObjectId::derived("scope", "b"),
        );
        let forbidden = Route::forbidden(a, b, BudgetRef::by_name("hop"));
        assert!(!AdoptionGate::new()
            .forbidding(&forbidden)
            .decide(&ledge())
            .adopted());

        let required = Route::required(a, b, BudgetRef::by_name("hop"));
        assert!(
            AdoptionGate::new()
                .forbidding(&required)
                .decide(&ledge())
                .adopted(),
            "a discovery that helps satisfy a required route is the emergence adoption exists for"
        );
    }

    #[test]
    fn an_invalidating_discovery_is_never_adopted() {
        let d = Discovery::Invalidating { edge: edge().0 };
        assert!(!d.is_additive());
        assert!(!AdoptionGate::new().decide(&d).adopted());
    }

    #[test]
    fn a_rejection_escalates_to_the_layer_that_can_close_the_route() {
        let got = AdoptionGate::new()
            .guarding(1, Verification::Breached { via: edge().1 })
            .decide(&ledge());
        let (f, r) = got.escalation(1).expect("a rejection escalates");
        assert_eq!(f, Failure::Breached { edge: 1 });
        assert_eq!(
            r,
            Response::Escalated {
                to: Layer::Geometry
            }
        );
        assert!(AdoptionGate::new().decide(&ledge()).escalation(1).is_none());
    }

    #[test]
    fn an_adopted_edge_counts_toward_looping_exactly_as_a_planned_one_does() {
        // Four scopes: a spanning tree has 3 edges; a fourth is one loop.
        let planned = cycle_density(4, 4);
        assert!(planned > 0.0);
        assert_eq!(
            cycle_density(4, 4),
            planned,
            "the density function knows nothing about provenance, which is the point"
        );
        assert_eq!(cycle_density(4, 3), 0.0, "a tree does not loop");
        assert_eq!(cycle_density(1, 0), 0.0);
        assert!(cycle_density(4, 99) <= 1.0, "clamped");
    }

    #[test]
    fn the_trace_row_says_adopted_or_why_not() {
        let (_, a, b) = edge();
        let adopted = DiscoveryTrace {
            from: b,
            to: a,
            slack: 0.4,
            outcome: AdoptionGate::new().decide(&ledge()),
        };
        assert!(adopted
            .to_string()
            .contains("discovered rather than planned"));

        let rejected = DiscoveryTrace {
            from: b,
            to: a,
            slack: 0.4,
            outcome: AdoptionGate::new()
                .guarding(1, Verification::Breached { via: a })
                .decide(&ledge()),
        };
        let line = rejected.to_string();
        assert!(line.contains("REJECTED"));
        assert!(line.contains("GUARDED"));
    }
}
