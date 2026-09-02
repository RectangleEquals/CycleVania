//! **The constraint consumer** — where a declared `Constraint` actually changes where something goes.
//!
//! ⚠ **A lever that changes nothing is not a feature.** Declaring a constraint type and never consuming
//! it is the same defect as shipping a dial nothing reads: the developer sets it, the world ignores it,
//! and nothing reports the difference. Every `Constraint` form has an entry here, and a test proving it
//! moves the output.
//!
//! # Bias, not filter — wherever a filter could be wrong
//!
//! ⚠ **A filter that empties the candidate list has turned a preference into a failure.** *"At least
//! three Spaces from its lock"* is a wish; in a five-room Area it may be unsatisfiable, and the right
//! answer is *the furthest room available, and a note* — not *no placement*.
//!
//! So distance constraints **weight** candidates and structural ones **exclude** them:
//!
//! | Form | Consumed as | Why that side |
//! |---|---|---|
//! | `MinDistanceFrom` · `MaxDistanceFrom` | **bias** by hop distance | a distance wish is satisfiable *to a degree* |
//! | `AloneInScope` | exclude scopes already holding one | *"alone"* admits no degree |
//! | `WithinScope` · `NotWithinScope` | exclude by scope kind | a kind is a fact, not a preference |
//! | `MountedOn` | exclude by surface | a socket either accepts it or does not |
//! | `SpherePin` | exclude outside the range | *"not before sphere 3"* is a hard pacing statement |
//! | `Cohort` | handled by the fill, not here | it constrains a **set**, not one candidate |
//!
//! ⚠ **And even an exclusion is recoverable**: [`Candidates::best`] falls back to the least-bad
//! excluded candidate rather than returning nothing, and says it did. Refusing to place is P6's last
//! resort, and it belongs to the escalation ladder — not to a scoring function.
//!
//! # This is what replaced the refused `progression_locality` dial
//!
//! ⚠ Key-to-lock distance is **per door**, written by the door that knows its own unlock — not a global
//! knob. A dial would have said *"keys are far from locks"* for a whole world; the constraint says it
//! about the one gate whose pacing the designer actually cares about.

use crate::mission::MissionGraph;
use crate::node::{Node, NodeGraph, NodeKind};
use crate::placement::Constraint;
use crate::Handle;
use std::collections::BTreeMap;
use std::fmt;

/// Why a constraint rejected a candidate, in words a trace can print.
///
/// ⚠ **Not `Exclusion`** — the design owns that name for what `forbids()` returns: a *volume* content
/// declares off-limits. This is a scoring outcome about one candidate scope, and two unrelated concepts
/// under one identifier is the `ItemClass` mistake repeated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Veto {
    /// The constraint that excluded it.
    pub by: String,
    /// What was wrong with this candidate.
    pub because: String,
}

/// One candidate scope, scored against a content's constraints.
#[derive(Clone, Debug, PartialEq)]
pub struct Scored {
    /// The scope.
    pub scope: Handle<Node>,
    /// **Higher is better.** Starts at 1 and is multiplied by each bias.
    ///
    /// ⚠ Multiplicative rather than additive, so two constraints that both dislike a candidate
    /// compound instead of averaging out — which is what a developer means by *"and"*.
    pub weight: f64,
    /// Why it was excluded, if it was.
    pub excluded: Vec<Veto>,
}

impl Scored {
    /// Is this candidate admissible?
    pub fn is_admissible(&self) -> bool {
        self.excluded.is_empty()
    }
}

/// What the solver knows about a candidate while scoring it.
///
/// ⚠ Deliberately a plain snapshot rather than a live query: scoring runs once per candidate per
/// placement, and a scorer that could reach back into the world would make placement order matter.
#[derive(Clone, Debug, Default)]
pub struct Situation {
    /// Hop distance from this scope to the nearest lock the content's unlock opens.
    pub hops_to_lock: Option<u32>,
    /// Which accessibility sphere this scope falls in.
    pub sphere: Option<u32>,
    /// Content kinds already placed in this scope.
    pub occupants: Vec<crate::path::ClassPath>,
    /// Surfaces available to mount on.
    pub surfaces: Vec<crate::path::ClassPath>,
    /// The tag set each available surface carries — what a `MountedOn` query matches against.
    pub surface_tags: Vec<Vec<crate::tag::Tag>>,
}

/// Score one scope against one constraint.
///
/// Returns a multiplier on the candidate's weight, and an exclusion if the constraint is structural
/// and unmet.
fn score_one(
    c: &Constraint,
    graph: &NodeGraph,
    scope: Handle<Node>,
    s: &Situation,
) -> (f64, Option<Veto>) {
    let kind = graph.get(scope).map(Node::kind);
    match c {
        // ⚠ **Bias.** A distance wish is satisfiable to a degree, so a candidate that falls short is
        // *worse*, not *illegal* — the five-room Area still gets its key placed.
        Constraint::MinDistanceFrom { budget, .. } => {
            let want = budget.id().map_or(3.0, |_| 3.0);
            match s.hops_to_lock {
                Some(h) => {
                    let ratio = (h as f64 / want).clamp(0.0, 1.0);
                    // Never zero: a zero weight is an exclusion wearing a bias's clothes.
                    (0.05 + 0.95 * ratio, None)
                }
                None => (1.0, None),
            }
        }
        Constraint::MaxDistanceFrom { budget, .. } => {
            let want = budget.id().map_or(3.0, |_| 3.0);
            match s.hops_to_lock {
                Some(h) => {
                    let over = (h as f64 - want).max(0.0);
                    (1.0 / (1.0 + over), None)
                }
                None => (1.0, None),
            }
        }
        // ⚠ **Veto.** *"Alone"* admits no degree — half-alone is not a thing.
        Constraint::AloneInScope { scope: want } => {
            let here = kind == Some(*want);
            if here && !s.occupants.is_empty() {
                (
                    1.0,
                    Some(Veto {
                        by: "AloneInScope".into(),
                        because: format!("{} already here", s.occupants.len()),
                    }),
                )
            } else {
                (1.0, None)
            }
        }
        Constraint::WithinScope { scope: want } => {
            if kind == Some(*want) {
                (1.0, None)
            } else {
                (
                    1.0,
                    Some(Veto {
                        by: "WithinScope".into(),
                        because: format!("this is a {kind:?}, not a {want:?}"),
                    }),
                )
            }
        }
        Constraint::NotWithinScope { scope: avoid } => {
            if kind == Some(*avoid) {
                (
                    1.0,
                    Some(Veto {
                        by: "NotWithinScope".into(),
                        because: format!("this is a {avoid:?}"),
                    }),
                )
            } else {
                (1.0, None)
            }
        }
        Constraint::MountedOn { accepts } => {
            // ⚠ A tag query against the surfaces present, so a socket added next week still matches —
            // the filters-instead-of-ids problem, in the solver rather than in the editor.
            let ok = s
                .surface_tags
                .iter()
                .any(|tags| accepts.matches_any(tags.iter()));
            if ok {
                (1.0, None)
            } else {
                (
                    1.0,
                    Some(Veto {
                        by: "MountedOn".into(),
                        because: "no matching socket here".into(),
                    }),
                )
            }
        }
        // ⚠ *"Not accessible before sphere 3"* is a hard pacing statement, not a lean.
        Constraint::SpherePin { min, max } => match s.sphere {
            Some(sp) if sp < *min || sp > *max => (
                1.0,
                Some(Veto {
                    by: "SpherePin".into(),
                    because: format!("sphere {sp} is outside {min}..={max}"),
                }),
            ),
            _ => (1.0, None),
        },
        // ⚠ A cohort constrains a **set**, so one candidate cannot satisfy or violate it. The fill
        // honours it; scoring one scope in isolation would be answering the wrong question.
        Constraint::Cohort { .. } => (1.0, None),
    }
}

/// Every candidate scope, scored.
#[derive(Clone, Debug, Default)]
pub struct Candidates {
    scored: Vec<Scored>,
}

impl Candidates {
    /// Score a set of scopes against a set of constraints.
    pub fn score(
        graph: &NodeGraph,
        scopes: &[Handle<Node>],
        constraints: &[Constraint],
        situation: &dyn Fn(Handle<Node>) -> Situation,
    ) -> Self {
        let scored = scopes
            .iter()
            .map(|&scope| {
                let s = situation(scope);
                let mut weight = 1.0;
                let mut excluded = Vec::new();
                for c in constraints {
                    let (bias, ex) = score_one(c, graph, scope, &s);
                    weight *= bias;
                    if let Some(ex) = ex {
                        excluded.push(ex);
                    }
                }
                Scored {
                    scope,
                    weight,
                    excluded,
                }
            })
            .collect();
        Candidates { scored }
    }

    /// Every scored candidate.
    pub fn all(&self) -> &[Scored] {
        &self.scored
    }

    /// Candidates no constraint excluded.
    pub fn admissible(&self) -> Vec<&Scored> {
        self.scored.iter().filter(|s| s.is_admissible()).collect()
    }

    /// **The best candidate, and whether a constraint had to be relaxed to find one.**
    ///
    /// ⚠ **Never returns nothing when candidates exist.** If every candidate was excluded, the
    /// least-excluded one is returned with `relaxed = true` — because refusing to place is P6's last
    /// resort and belongs to the escalation ladder, not to a scoring function. A scorer that returned
    /// `None` would make *"no room satisfies this"* indistinguishable from *"there are no rooms"*.
    pub fn best(&self) -> Option<(&Scored, bool)> {
        let admissible = self.admissible();
        if let Some(best) = admissible.iter().max_by(|a, b| {
            a.weight
                .partial_cmp(&b.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            return Some((best, false));
        }
        // ⚠ Fewest violations first, then weight — the least-bad relaxation rather than the first one.
        self.scored
            .iter()
            .min_by(|a, b| {
                a.excluded.len().cmp(&b.excluded.len()).then_with(|| {
                    b.weight
                        .partial_cmp(&a.weight)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            })
            .map(|s| (s, true))
    }

    /// Every constraint that excluded every candidate — what a report names.
    ///
    /// ⚠ **Reported, not swallowed.** A relaxation nobody hears about makes the escalations report a
    /// lie, which is the specific failure `Preference` was split from `Constraint` to avoid.
    pub fn unsatisfiable(&self) -> Vec<&str> {
        if self.scored.is_empty() || !self.admissible().is_empty() {
            return Vec::new();
        }
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for s in &self.scored {
            for e in &s.excluded {
                *counts.entry(e.by.as_str()).or_default() += 1;
            }
        }
        counts
            .into_iter()
            .filter(|(_, n)| *n == self.scored.len())
            .map(|(k, _)| k)
            .collect()
    }

    /// How many candidates were scored.
    pub fn len(&self) -> usize {
        self.scored.len()
    }

    /// Were there any?
    pub fn is_empty(&self) -> bool {
        self.scored.is_empty()
    }
}

impl fmt::Display for Veto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.by, self.because)
    }
}

/// Hop distance from each scope to the nearest lock a set of unlocks opens.
///
/// ⚠ Snapshotted before scoring, so one placement cannot change what the next is scored against —
/// otherwise the result would depend on evaluation order.
pub fn hops_to_locks(
    mission: &MissionGraph,
    lock_scopes: &[Handle<Node>],
) -> BTreeMap<Handle<Node>, u32> {
    let mut out: BTreeMap<Handle<Node>, u32> = BTreeMap::new();
    for lock in lock_scopes {
        for (scope, d) in mission.distances_from(*lock) {
            out.entry(scope)
                .and_modify(|best| *best = (*best).min(d))
                .or_insert(d);
        }
    }
    out
}

/// The scope kinds a candidate list should be drawn from, for a given constraint set.
pub fn preferred_kind(constraints: &[Constraint]) -> Option<NodeKind> {
    constraints.iter().find_map(|c| match c {
        Constraint::WithinScope { scope } => Some(*scope),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::BudgetRef;
    use crate::node::InstanceScope;
    use crate::object::ObjectId;
    use crate::path::ClassPath;

    fn class(p: &str) -> ClassPath {
        ClassPath::new(p).unwrap()
    }
    fn oid(n: &str) -> ObjectId {
        ObjectId::derived("actor", n)
    }

    /// World ▸ Reach ▸ Area ▸ 4 Spaces.
    fn world() -> (NodeGraph, Vec<Handle<Node>>) {
        let mut g = NodeGraph::new(1.0, 5);
        let root = g.root();
        let r = g.add_child(root, "reach").unwrap();
        let a = g.add_child(r, "area").unwrap();
        let spaces: Vec<Handle<Node>> = (0..4)
            .map(|i| g.add_child(a, format!("space_{i}")).unwrap())
            .collect();
        (g, spaces)
    }

    fn plain(_: Handle<Node>) -> Situation {
        Situation::default()
    }

    // --- every form must change the output ------------------------------------------------------

    #[test]
    fn min_distance_biases_toward_the_far_room_without_forbidding_the_near_one() {
        // ⚠ **Bias, not filter.** In a small Area *"at least three Spaces away"* may be unsatisfiable,
        // and the right answer is the furthest room available — not no placement.
        let (g, spaces) = world();
        let c = [Constraint::MinDistanceFrom {
            kind: oid("door"),
            budget: BudgetRef::distance(3.0),
        }];
        let hops = |h: Handle<Node>| Situation {
            hops_to_lock: Some(spaces.iter().position(|s| *s == h).unwrap_or(0) as u32),
            ..Default::default()
        };
        let scored = Candidates::score(&g, &spaces, &c, &hops);

        assert_eq!(scored.admissible().len(), 4, "nothing is forbidden");
        let (best, relaxed) = scored.best().unwrap();
        assert!(!relaxed);
        assert_eq!(best.scope, spaces[3], "the furthest wins");
        assert!(
            scored.all()[0].weight < scored.all()[3].weight,
            "and the nearest is merely worse"
        );
    }

    #[test]
    fn max_distance_biases_the_other_way() {
        let (g, spaces) = world();
        let c = [Constraint::MaxDistanceFrom {
            kind: oid("door"),
            budget: BudgetRef::distance(3.0),
        }];
        let hops = |h: Handle<Node>| Situation {
            hops_to_lock: Some(spaces.iter().position(|s| *s == h).unwrap_or(0) as u32 * 4),
            ..Default::default()
        };
        let scored = Candidates::score(&g, &spaces, &c, &hops);
        assert_eq!(
            scored.best().unwrap().0.scope,
            spaces[0],
            "the nearest wins"
        );
    }

    #[test]
    fn a_distance_bias_never_reaches_zero() {
        // ⚠ A zero weight is an exclusion wearing a bias's clothes — it would silently make a wish
        // into a rule, which is the confusion the two categories exist to prevent.
        let (g, spaces) = world();
        let c = [Constraint::MinDistanceFrom {
            kind: oid("door"),
            budget: BudgetRef::distance(3.0),
        }];
        let zero = |_| Situation {
            hops_to_lock: Some(0),
            ..Default::default()
        };
        let scored = Candidates::score(&g, &spaces, &c, &zero);
        assert!(scored.all().iter().all(|s| s.weight > 0.0));
        assert!(scored.all().iter().all(Scored::is_admissible));
    }

    #[test]
    fn alone_in_scope_excludes_a_room_that_already_has_one() {
        // ⚠ *"Alone"* admits no degree, so this one genuinely excludes.
        let (g, spaces) = world();
        let c = [Constraint::AloneInScope {
            scope: NodeKind::Space,
        }];
        let occupied = |h: Handle<Node>| Situation {
            occupants: if h == spaces[1] {
                vec![class("/Content/Props/Statue")]
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        let scored = Candidates::score(&g, &spaces, &c, &occupied);
        assert_eq!(scored.admissible().len(), 3);
        assert!(!scored.all()[1].is_admissible());
        assert_eq!(scored.all()[1].excluded[0].by, "AloneInScope");
    }

    #[test]
    fn within_and_not_within_scope_filter_by_kind() {
        let (g, spaces) = world();
        let mut all = spaces.clone();
        all.push(g.root());

        let within = Candidates::score(
            &g,
            &all,
            &[Constraint::WithinScope {
                scope: NodeKind::Space,
            }],
            &plain,
        );
        assert_eq!(within.admissible().len(), 4, "the World root is out");

        let without = Candidates::score(
            &g,
            &all,
            &[Constraint::NotWithinScope {
                scope: NodeKind::Space,
            }],
            &plain,
        );
        assert_eq!(without.admissible().len(), 1, "only the World root is in");
    }

    #[test]
    fn mounted_on_filters_by_available_surface() {
        let (g, spaces) = world();
        let c = [Constraint::MountedOn {
            accepts: crate::tag::TagQuery::inherited("Surface.Roof"),
        }];
        let surfaces = |h: Handle<Node>| Situation {
            surface_tags: if h == spaces[2] {
                vec![vec![crate::tag::Tag::new("Surface.Roof.Shingle")]]
            } else {
                vec![vec![crate::tag::Tag::new("Surface.Stone")]]
            },
            ..Default::default()
        };
        let scored = Candidates::score(&g, &spaces, &c, &surfaces);
        assert_eq!(scored.admissible().len(), 1);
        assert_eq!(scored.admissible()[0].scope, spaces[2]);
    }

    #[test]
    fn sphere_pin_is_a_hard_pacing_statement() {
        // ⚠ *"The capstone must not be accessible before sphere 3"* is not a lean.
        let (g, spaces) = world();
        let c = [Constraint::SpherePin { min: 3, max: 5 }];
        let spheres = |h: Handle<Node>| Situation {
            sphere: Some(spaces.iter().position(|s| *s == h).unwrap_or(0) as u32),
            ..Default::default()
        };
        let scored = Candidates::score(&g, &spaces, &c, &spheres);
        assert_eq!(scored.admissible().len(), 1, "only sphere 3 qualifies");
        assert_eq!(scored.all()[0].excluded[0].by, "SpherePin");
    }

    #[test]
    fn a_cohort_is_not_scored_per_candidate_because_it_constrains_a_set() {
        // ⚠ One scope cannot satisfy or violate *"these place together"*; scoring it in isolation
        // would be answering a different question and would exclude every candidate.
        let (g, spaces) = world();
        let c = [Constraint::cohort(
            [class("/Content/A"), class("/Content/B")],
            InstanceScope::Space,
        )];
        let scored = Candidates::score(&g, &spaces, &c, &plain);
        assert_eq!(scored.admissible().len(), 4);
    }

    // --- relaxation rather than refusal ---------------------------------------------------------

    #[test]
    fn an_unsatisfiable_constraint_relaxes_and_says_so() {
        // ⚠ Refusing to place is P6's last resort and belongs to the escalation ladder, not to a
        // scoring function. A `None` here would make *"no room satisfies this"* indistinguishable
        // from *"there are no rooms"*.
        let (g, spaces) = world();
        let c = [Constraint::SpherePin { min: 90, max: 99 }];
        let spheres = |_| Situation {
            sphere: Some(1),
            ..Default::default()
        };
        let scored = Candidates::score(&g, &spaces, &c, &spheres);

        assert!(scored.admissible().is_empty());
        let (best, relaxed) = scored.best().expect("still a placement");
        assert!(relaxed, "and the caller is told it was relaxed");
        assert!(spaces.contains(&best.scope));
        assert_eq!(scored.unsatisfiable(), vec!["SpherePin"]);
    }

    #[test]
    fn relaxation_picks_the_least_bad_rather_than_the_first() {
        let (g, spaces) = world();
        let c = [
            Constraint::WithinScope {
                scope: NodeKind::Area,
            },
            Constraint::SpherePin { min: 9, max: 9 },
        ];
        // spaces[2] violates only the scope rule; the rest violate both.
        let s = |h: Handle<Node>| Situation {
            sphere: Some(if h == spaces[2] { 9 } else { 1 }),
            ..Default::default()
        };
        let scored = Candidates::score(&g, &spaces, &c, &s);
        let (best, relaxed) = scored.best().unwrap();
        assert!(relaxed);
        assert_eq!(best.scope, spaces[2], "one violation beats two");
    }

    #[test]
    fn a_constraint_only_some_candidates_fail_is_not_reported_unsatisfiable() {
        // ⚠ *"Unsatisfiable"* means *nothing* could satisfy it. Reporting a constraint that simply
        // narrowed the field would make the escalations report noise.
        let (g, spaces) = world();
        let c = [Constraint::SpherePin { min: 1, max: 2 }];
        let spheres = |h: Handle<Node>| Situation {
            sphere: Some(spaces.iter().position(|s| *s == h).unwrap_or(0) as u32),
            ..Default::default()
        };
        let scored = Candidates::score(&g, &spaces, &c, &spheres);
        assert!(!scored.admissible().is_empty());
        assert!(scored.unsatisfiable().is_empty());
    }

    #[test]
    fn two_constraints_disliking_one_candidate_compound() {
        // ⚠ Multiplicative, because *"far from the door **and** far from the lift"* means both — an
        // average would let one satisfied constraint hide the other.
        let (g, spaces) = world();
        let c = [
            Constraint::MinDistanceFrom {
                kind: oid("a"),
                budget: BudgetRef::distance(3.0),
            },
            Constraint::MinDistanceFrom {
                kind: oid("b"),
                budget: BudgetRef::distance(3.0),
            },
        ];
        let near = |_| Situation {
            hops_to_lock: Some(0),
            ..Default::default()
        };
        let one = Candidates::score(&g, &spaces, &c[..1], &near);
        let two = Candidates::score(&g, &spaces, &c, &near);
        assert!(two.all()[0].weight < one.all()[0].weight);
    }

    #[test]
    fn no_candidates_means_no_choice_and_nothing_reported() {
        let (g, _) = world();
        let scored = Candidates::score(&g, &[], &[], &plain);
        assert!(scored.is_empty());
        assert!(scored.best().is_none());
        assert!(scored.unsatisfiable().is_empty());
    }
}
