//! **L0 + L1** — resolving what content exists into what the world should aim to contain.
//!
//! # What L1 decides, and what it does not
//!
//! L1 produces **targets, not placements**. It answers "this room should aim for about three things,
//! drawn from these five candidates, weighted like so" — and then L2 has final say, because
//! solvability outranks aesthetics. A schedule that cannot be honoured is not an error; it is a
//! preference the solver declined.
//!
//! Keeping that boundary sharp is what stops the two layers fighting. L1 never asks whether a world is
//! solvable, and L2 never asks whether it is well-paced.
//!
//! # Whose count is `AdaptiveRange`?
//!
//! The design specifies the formula but leaves one thing implicit, and it matters: an
//! [`AdaptiveRange`] belongs to a **slot** (how many things go in this room), not to a piece of
//! content. That follows from the formula itself — `unique` counts *the distinct pieces eligible for
//! this slot*, which is a property of the slot, not of any one candidate.
//!
//! Content declares its own eligibility ([`Schedule`]); per-scope limits are a *constraint*
//! (`Constraint::MaxPerScope`), enforced by L2/L3 where placement actually happens.
//!
//! # The inputs a dev has
//!
//! Adaptive counting is the **default**, not a mandate. Each knob answers a different question, so
//! tuning one never disturbs another:
//!
//! | Input | Question it answers | Lives on |
//! |---|---|---|
//! | [`Span`] | *where* may this appear? | content |
//! | [`Curve`] weight | *how favoured* is it against its peers? | content |
//! | [`Curve`] chance | *how likely* is it to be offered at all? | content |
//! | [`ScopeFilter`] | *what kind of scope* can hold it? | content |
//! | [`WorldLimit`] | *how many* may exist in the entire world? | content |
//! | [`CountRule`] | *how many* go in this slot, and by what rule? | slot |
//!
//! [`CountRule`] is where a dev opts out of adaptation entirely — `Fixed` for "exactly one puzzle per
//! chamber", `Range` for variation without adaptation, `Curve` for density that follows depth. All
//! four report their reasoning, so choosing control does not cost you the explanation.
//!
//! [`WorldLimit`] exists because a per-slot count cannot express a world-wide fact. "Exactly one final
//! boss" has no per-room formulation; L1 records the demand and L2 honours it.
//!
//! # The adaptive part
//!
//! A fixed "put 5 things in every room" degrades badly: with two distinct pieces available it produces
//! obvious repetition, and with fifty it wastes them. So the target tracks **how much distinct content
//! is actually placeable**:
//!
//! ```text
//! supported   = floor(unique × repeat_tol × weight)
//! soft_target = min(hard_max, supported) + seeded_jitter
//! ```
//!
//! * **Abundant** (`supported ≥ hard_max`) — variety fills the ceiling; sit at `hard_max`.
//! * **Moderate** — take `supported`.
//! * **Scarce** (`supported < soft_min`) — take `supported` anyway, **honestly falling below
//!   `soft_min`**. The room reads sparse rather than repetitive or broken.
//!
//! That last case is the whole point, and it is why **`soft_min` is a preference, not a floor**. Only
//! `hard_max` is a true ceiling. A generator that padded up to `soft_min` with whatever was left would
//! produce exactly the repetition the formula exists to avoid.

use crate::content::{ContentKind, ContentRegistry};
use crate::node::{Node, NodeGraph, NodeKind};
use crate::object::{Object, ObjectId};
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use crate::Handle;
use cv_determinism::{math, Rng};
use std::collections::BTreeMap;
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Progression, spans and curves
// ---------------------------------------------------------------------------------------------

/// How far through the world a scope sits, normalised to `[0, 1]`.
///
/// At M08 this is depth — a scope's Reach index over the Reach count. M12 generalises it to a
/// pluggable `ProgressionAxis`, which is why callers take a `Progression` rather than computing a
/// ratio inline: there will be one place to change.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Progression(f64);

impl Progression {
    /// The start of the world.
    pub const START: Progression = Progression(0.0);
    /// The end of the world.
    pub const END: Progression = Progression(1.0);

    /// Clamped to `[0, 1]`.
    pub fn new(v: f64) -> Self {
        Progression(math::saturate(v))
    }

    /// The underlying fraction.
    pub fn value(self) -> f64 {
        self.0
    }
}

/// The stretch of progression over which content is eligible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    start: f64,
    end: f64,
}

impl Span {
    /// Eligible everywhere.
    pub const ALWAYS: Span = Span {
        start: 0.0,
        end: 1.0,
    };

    /// Eligible over `[start, end]`. Reversed inputs are sorted rather than rejected.
    pub fn new(start: f64, end: f64) -> Self {
        let (a, b) = (math::saturate(start), math::saturate(end));
        Span {
            start: math::min(a, b),
            end: math::max(a, b),
        }
    }

    /// Eligible from `start` onwards.
    pub fn from(start: f64) -> Self {
        Span::new(start, 1.0)
    }

    /// Eligible until `end`.
    pub fn until(end: f64) -> Self {
        Span::new(0.0, end)
    }

    /// Where it begins.
    pub fn start(self) -> f64 {
        self.start
    }

    /// Where it ends.
    pub fn end(self) -> f64 {
        self.end
    }

    /// Is this progression inside the span? Inclusive at both ends.
    pub fn contains(self, p: Progression) -> bool {
        p.0 >= self.start && p.0 <= self.end
    }
}

impl Default for Span {
    fn default() -> Self {
        Span::ALWAYS
    }
}

/// A piecewise-linear curve over progression — the "weight over time" a dev draws in the editor.
///
/// Deliberately simple: keyframes with linear interpolation, clamped outside the range. Richer
/// interpolation belongs with the editor's curve tool (M25), where a dev can see what they are
/// getting; inventing spline modes nobody can preview would be guessing.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    /// Keyframes, sorted by x.
    points: Vec<(f64, f64)>,
}

impl Curve {
    /// The same value everywhere.
    pub fn constant(v: f64) -> Self {
        Curve {
            points: vec![(0.0, v)],
        }
    }

    /// From keyframes. Sorted on construction, so callers need not.
    pub fn from_points(points: impl IntoIterator<Item = (f64, f64)>) -> Self {
        let mut points: Vec<(f64, f64)> = points.into_iter().collect();
        if points.is_empty() {
            return Curve::constant(0.0);
        }
        // Sorting here (rather than trusting input) keeps evaluation branch-free and makes the curve
        // canonical, so two curves built from the same keys in different orders are equal.
        points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        Curve { points }
    }

    /// A straight line from `at_start` to `at_end`.
    pub fn ramp(at_start: f64, at_end: f64) -> Self {
        Curve::from_points([(0.0, at_start), (1.0, at_end)])
    }

    /// The keyframes.
    pub fn points(&self) -> &[(f64, f64)] {
        &self.points
    }

    /// Evaluate at a progression. Clamped outside the keyed range.
    pub fn eval(&self, p: Progression) -> f64 {
        let x = p.value();
        match self.points.first() {
            None => 0.0,
            Some(first) if x <= first.0 => first.1,
            _ => {
                let last = self.points[self.points.len() - 1];
                if x >= last.0 {
                    return last.1;
                }
                // Linear scan: curves have a handful of keys, so this beats a binary search and keeps
                // the traversal order obvious.
                for w in self.points.windows(2) {
                    let (x0, y0) = w[0];
                    let (x1, y1) = w[1];
                    if x <= x1 {
                        if x1 == x0 {
                            return y1;
                        }
                        return math::lerp(y0, y1, (x - x0) / (x1 - x0));
                    }
                }
                last.1
            }
        }
    }
}

impl Default for Curve {
    fn default() -> Self {
        Curve::constant(1.0)
    }
}

// ---------------------------------------------------------------------------------------------
// AdaptiveRange
// ---------------------------------------------------------------------------------------------

/// How the target count for a slot degrades with available variety.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveRange {
    /// The count below which a slot reads sparse. A **preference**, not a floor.
    pub soft_min: u32,
    /// The true ceiling — never exceeded.
    pub hard_max: u32,
    /// How often one piece may repeat before it reads repetitive. A dial; ~1.5 by default.
    pub repeat_tol: f64,
    /// Maximum seeded wobble applied to the target, in either direction.
    pub jitter: u32,
}

/// Which regime the target landed in — the headline of "why this number?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetOutcome {
    // --- adaptive ---
    /// Variety exceeded the ceiling; the target sits at `hard_max`.
    Abundant,
    /// Between `soft_min` and `hard_max`; the target tracks what is supported.
    Moderate,
    /// Below `soft_min`. The slot reads **sparse rather than repetitive** — deliberate, not a failure.
    Scarce,
    // --- the dev took control ---
    /// A constant the dev set. Not adaptive by choice.
    Fixed,
    /// Sampled uniformly from a fixed range.
    Sampled,
    /// Read from a curve over progression.
    Curved,
}

impl TargetOutcome {
    /// Did variety influence this number?
    ///
    /// `false` means the dev overrode the adaptive behaviour, so a sparse-looking result is intended
    /// rather than something to go fix.
    pub fn is_adaptive(self) -> bool {
        matches!(
            self,
            TargetOutcome::Abundant | TargetOutcome::Moderate | TargetOutcome::Scarce
        )
    }
}

impl fmt::Display for TargetOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TargetOutcome::Abundant => "abundant",
            TargetOutcome::Moderate => "moderate",
            TargetOutcome::Scarce => "scarce",
            TargetOutcome::Fixed => "fixed",
            TargetOutcome::Sampled => "sampled",
            TargetOutcome::Curved => "curved",
        })
    }
}

/// The full derivation of a target, kept so a dev can ask "why three?" and get an answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetReasoning {
    /// Distinct eligible pieces for this slot.
    pub unique: u32,
    /// The reuse tolerance in force.
    pub repeat_tol: f64,
    /// The slot's schedule weight here.
    pub weight: f64,
    /// `floor(unique × repeat_tol × weight)`.
    pub supported: u32,
    /// The seeded wobble actually applied.
    pub jitter: i32,
    /// Which regime it landed in.
    pub outcome: TargetOutcome,
    /// The final target.
    pub target: u32,
}

impl fmt::Display for TargetReasoning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} → {} ({} unique × {:.2} repeat_tol × {:.2} weight = {} supported, jitter {:+})",
            self.outcome,
            self.target,
            self.unique,
            self.repeat_tol,
            self.weight,
            self.supported,
            self.jitter
        )
    }
}

impl AdaptiveRange {
    /// A range with the default reuse tolerance and no jitter.
    pub fn new(soft_min: u32, hard_max: u32) -> Self {
        AdaptiveRange {
            soft_min,
            hard_max,
            repeat_tol: 1.5,
            jitter: 0,
        }
    }

    /// Set the reuse tolerance.
    pub fn with_repeat_tol(mut self, repeat_tol: f64) -> Self {
        self.repeat_tol = math::max(0.0, repeat_tol);
        self
    }

    /// Set the seeded wobble.
    pub fn with_jitter(mut self, jitter: u32) -> Self {
        self.jitter = jitter;
        self
    }

    /// Compute the target for a slot, and record how.
    ///
    /// `rng` should be forked per slot so the jitter is reproducible and independent of the order
    /// slots are visited.
    pub fn resolve(&self, unique: u32, weight: f64, rng: &mut Rng) -> TargetReasoning {
        let weight = math::saturate(weight);
        let supported = math::floor(unique as f64 * self.repeat_tol * weight);
        // Saturating, not wrapping: an absurd repeat_tol should pin at the ceiling, not overflow.
        let supported = if supported >= u32::MAX as f64 {
            u32::MAX
        } else {
            supported as u32
        };

        let outcome = if supported >= self.hard_max {
            TargetOutcome::Abundant
        } else if supported >= self.soft_min {
            TargetOutcome::Moderate
        } else {
            TargetOutcome::Scarce
        };

        let base = supported.min(self.hard_max);
        let jitter = if self.jitter == 0 {
            0
        } else {
            // Symmetric in [-jitter, +jitter].
            rng.range_i64(-(self.jitter as i64), self.jitter as i64 + 1) as i32
        };

        // Jitter may not breach the ceiling, and cannot push a count negative.
        let target = (base as i64 + jitter as i64).clamp(0, self.hard_max as i64) as u32;

        TargetReasoning {
            unique,
            repeat_tol: self.repeat_tol,
            weight,
            supported,
            jitter,
            outcome,
            target,
        }
    }
}

impl Default for AdaptiveRange {
    fn default() -> Self {
        AdaptiveRange::new(0, 4)
    }
}

/// **How a slot's target is decided.** Adaptive by default; the rest are opt-in dev control.
///
/// Adaptation is the right default because it degrades gracefully, but it is not always what a dev
/// wants. A Portal-style chamber holding *exactly one* puzzle should hold exactly one regardless of
/// how much content happens to be available; a boss arena should not gain a second boss because the
/// library grew. So the escape hatches are first-class rather than something to work around:
///
/// | Rule | Use it when |
/// |---|---|
/// | [`CountRule::Fixed`] | the number is part of the design ("one puzzle per chamber") |
/// | [`CountRule::Range`] | you want variation but not adaptation |
/// | [`CountRule::Curve`] | density should follow progression ("busier as you go deeper") |
/// | [`CountRule::Adaptive`] | you want the count to track available variety (the default) |
///
/// All four are deterministic and all four report their reasoning, so switching between them does not
/// cost you the explanation of what happened.
#[derive(Clone, Debug, PartialEq)]
pub enum CountRule {
    /// Exactly this many, every time.
    Fixed(u32),
    /// Uniformly sampled from `[min, max]`, ignoring how much content exists.
    ///
    /// The design's "plain range" escape hatch: variation without adaptation.
    Range { min: u32, max: u32 },
    /// Read from a curve over progression and rounded — density as a function of depth.
    Curve(Curve),
    /// Tracks available variety. See [`AdaptiveRange`].
    Adaptive(AdaptiveRange),
}

impl CountRule {
    /// A constant.
    pub fn fixed(n: u32) -> Self {
        CountRule::Fixed(n)
    }

    /// A fixed range, sorted so reversed bounds are not an error.
    pub fn range(min: u32, max: u32) -> Self {
        CountRule::Range {
            min: min.min(max),
            max: min.max(max),
        }
    }

    /// A curve over progression.
    pub fn curve(curve: Curve) -> Self {
        CountRule::Curve(curve)
    }

    /// An adaptive range.
    pub fn adaptive(range: AdaptiveRange) -> Self {
        CountRule::Adaptive(range)
    }

    /// Decide the target, and record how.
    ///
    /// `unique` and `weight` are always recorded even when a rule ignores them, so a dev switching a
    /// slot from `Fixed` to `Adaptive` can see what the adaptive answer *would* have been.
    pub fn resolve(
        &self,
        unique: u32,
        weight: f64,
        progression: Progression,
        rng: &mut Rng,
    ) -> TargetReasoning {
        match self {
            CountRule::Adaptive(range) => range.resolve(unique, weight, rng),
            CountRule::Fixed(n) => TargetReasoning {
                unique,
                repeat_tol: 0.0,
                weight,
                supported: *n,
                jitter: 0,
                outcome: TargetOutcome::Fixed,
                target: *n,
            },
            CountRule::Range { min, max } => {
                let target = if min == max {
                    *min
                } else {
                    rng.range_u64(*min as u64, *max as u64 + 1) as u32
                };
                TargetReasoning {
                    unique,
                    repeat_tol: 0.0,
                    weight,
                    supported: target,
                    jitter: 0,
                    outcome: TargetOutcome::Sampled,
                    target,
                }
            }
            CountRule::Curve(curve) => {
                let raw = curve.eval(progression);
                // Round, then clamp at zero — a curve dipping negative means "none here".
                let target = math::max(0.0, math::round(raw)) as u32;
                TargetReasoning {
                    unique,
                    repeat_tol: 0.0,
                    weight,
                    supported: target,
                    jitter: 0,
                    outcome: TargetOutcome::Curved,
                    target,
                }
            }
        }
    }
}

impl Default for CountRule {
    fn default() -> Self {
        CountRule::Adaptive(AdaptiveRange::default())
    }
}

impl From<AdaptiveRange> for CountRule {
    fn from(r: AdaptiveRange) -> Self {
        CountRule::Adaptive(r)
    }
}

// ---------------------------------------------------------------------------------------------
// Scope applicability and world-wide limits
// ---------------------------------------------------------------------------------------------

/// Which scope kinds a piece of content may fill.
///
/// Stored as a bitmask over [`NodeKind`] — compact, order-free, and cheap to test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ScopeFilter(u8);

impl ScopeFilter {
    /// Any scope kind.
    pub const ANY: ScopeFilter = ScopeFilter(0b1_1111);

    /// Only the given kinds.
    pub fn only(kinds: impl IntoIterator<Item = NodeKind>) -> Self {
        let mut bits = 0u8;
        for k in kinds {
            bits |= 1 << k.depth();
        }
        ScopeFilter(bits)
    }

    /// May this content fill that kind of scope?
    pub fn allows(self, kind: NodeKind) -> bool {
        self.0 & (1 << kind.depth()) != 0
    }

    /// Does this permit nothing?
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The permitted kinds, outermost first.
    pub fn kinds(self) -> impl Iterator<Item = NodeKind> {
        NodeKind::ALL.into_iter().filter(move |k| self.allows(*k))
    }
}

/// How many of something may exist across the **whole world**.
///
/// Per-slot counts cannot express "exactly one final boss" or "at most three save rooms" — those are
/// world-wide facts, and without them a unique artifact silently becomes several. L1 records the
/// limit; L2 is what honours it, since only L2 places anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldLimit {
    /// The world must contain at least this many. `0` means optional.
    pub min: u32,
    /// The world may contain at most this many. `None` means unbounded.
    pub max: Option<u32>,
}

impl WorldLimit {
    /// No world-wide restriction.
    pub const UNLIMITED: WorldLimit = WorldLimit { min: 0, max: None };

    /// Exactly `n` — the unique-artifact case.
    pub fn exactly(n: u32) -> Self {
        WorldLimit {
            min: n,
            max: Some(n),
        }
    }

    /// At most `n`.
    pub fn at_most(n: u32) -> Self {
        WorldLimit {
            min: 0,
            max: Some(n),
        }
    }

    /// At least `n`.
    pub fn at_least(n: u32) -> Self {
        WorldLimit { min: n, max: None }
    }

    /// Must the world contain this?
    ///
    /// L2 treats a required piece as a **demand**, not a preference: failing to place it is a failed
    /// generation, not a sparse one.
    pub fn is_required(self) -> bool {
        self.min > 0
    }

    /// Is `count` acceptable?
    pub fn permits(self, count: u32) -> bool {
        count >= self.min && self.max.is_none_or(|m| count <= m)
    }
}

impl Default for WorldLimit {
    fn default() -> Self {
        WorldLimit::UNLIMITED
    }
}

// ---------------------------------------------------------------------------------------------
// Schedules
// ---------------------------------------------------------------------------------------------

/// One piece of content's eligibility and bias.
///
/// Data, not behaviour — the editor's scheduling panel and a preset's config both tune this, so it has
/// to be inspectable and serializable rather than compiled into a callback.
///
/// **Weight and chance are different, and conflating them is the common mistake.** Weight decides
/// *which* candidate wins when several compete; chance decides whether a candidate enters the running
/// at all. A boss with weight `1.0` and chance `0.1` is strongly preferred on the rare occasions it
/// appears — which no single number can express.
#[derive(Clone, Debug, PartialEq)]
pub struct Schedule {
    /// Where in the world this is eligible.
    pub span: Span,
    /// How strongly it is favoured, over progression.
    pub weight: Curve,
    /// Probability of being offered at a slot, over progression. `1.0` means always.
    pub chance: Curve,
    /// Which scope kinds may hold it.
    pub scopes: ScopeFilter,
    /// How many may exist in the whole world.
    pub world_limit: WorldLimit,
}

impl Schedule {
    /// Eligible everywhere, at full weight, in any scope.
    pub fn always() -> Self {
        Schedule {
            span: Span::ALWAYS,
            weight: Curve::constant(1.0),
            chance: Curve::constant(1.0),
            scopes: ScopeFilter::ANY,
            world_limit: WorldLimit::UNLIMITED,
        }
    }

    /// The sensible default for a kind of content — notably restricting it to its natural scopes.
    pub fn for_kind(kind: ContentKind) -> Self {
        Schedule {
            scopes: ScopeFilter::only(kind.default_scopes().iter().copied()),
            ..Schedule::always()
        }
    }

    /// Eligible over a span, at full weight.
    pub fn during(span: Span) -> Self {
        Schedule {
            span,
            ..Schedule::always()
        }
    }

    /// Set the weight curve.
    pub fn weighted(mut self, weight: Curve) -> Self {
        self.weight = weight;
        self
    }

    /// Set a constant probability of being offered.
    pub fn with_chance(mut self, chance: f64) -> Self {
        self.chance = Curve::constant(math::saturate(chance));
        self
    }

    /// Set a probability curve over progression.
    pub fn with_chance_curve(mut self, chance: Curve) -> Self {
        self.chance = chance;
        self
    }

    /// Restrict which scope kinds may hold this.
    pub fn in_scopes(mut self, kinds: impl IntoIterator<Item = NodeKind>) -> Self {
        self.scopes = ScopeFilter::only(kinds);
        self
    }

    /// Set the world-wide limit.
    pub fn limited(mut self, limit: WorldLimit) -> Self {
        self.world_limit = limit;
        self
    }

    /// Weight here, or `0.0` outside the span.
    pub fn weight_at(&self, p: Progression) -> f64 {
        if self.span.contains(p) {
            math::max(0.0, self.weight.eval(p))
        } else {
            0.0
        }
    }

    /// Probability of being offered here, clamped to `[0, 1]`.
    pub fn chance_at(&self, p: Progression) -> f64 {
        if self.span.contains(p) {
            math::saturate(self.chance.eval(p))
        } else {
            0.0
        }
    }

    /// Is this eligible here, with non-zero weight? Ignores [`Schedule::chance`], which is a roll.
    pub fn is_eligible(&self, p: Progression) -> bool {
        self.weight_at(p) > 0.0
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule::always()
    }
}

/// Per-content schedules, plus the fallback for content that declares none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScheduleBook {
    schedules: BTreeMap<ObjectId, Schedule>,
}

impl ScheduleBook {
    /// An empty book — every content falls back to [`Schedule::always`].
    pub fn new() -> Self {
        ScheduleBook::default()
    }

    /// Set a content's schedule.
    pub fn set(&mut self, content: ObjectId, schedule: Schedule) -> &mut Self {
        self.schedules.insert(content, schedule);
        self
    }

    /// A content's schedule, or the default.
    pub fn get(&self, content: ObjectId) -> Schedule {
        self.schedules.get(&content).cloned().unwrap_or_default()
    }

    /// Is a schedule explicitly set?
    pub fn contains(&self, content: ObjectId) -> bool {
        self.schedules.contains_key(&content)
    }

    /// How many explicit schedules.
    pub fn len(&self) -> usize {
        self.schedules.len()
    }

    /// Are there none?
    pub fn is_empty(&self) -> bool {
        self.schedules.is_empty()
    }
}

/// How many things a kind of scope should aim to hold.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotRule {
    /// The scope kind this fills.
    pub scope_kind: NodeKind,
    /// How the target is decided — adaptive, or one of the dev-controlled alternatives.
    pub count: CountRule,
}

impl SlotRule {
    /// A rule for a scope kind. Accepts an [`AdaptiveRange`] or any [`CountRule`].
    pub fn new(scope_kind: NodeKind, count: impl Into<CountRule>) -> Self {
        SlotRule {
            scope_kind,
            count: count.into(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// L0 — the content pool
// ---------------------------------------------------------------------------------------------

/// One schedulable piece of content, resolved with its schedule.
#[derive(Clone, Debug, PartialEq)]
pub struct PoolEntry {
    /// Which content.
    pub content: ObjectId,
    /// What sort it is.
    pub kind: ContentKind,
    /// Its eligibility and bias.
    pub schedule: Schedule,
}

/// **L0's output** — everything the scheduler may draw on.
///
/// Only [`ContentKind::is_schedulable`] content appears. A `Component` or a `StaticMesh` exists and is
/// referenced, but is never something the algorithm places on its own, so it is filtered here once
/// rather than checked at every later site.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContentPool {
    entries: Vec<PoolEntry>,
}

impl ContentPool {
    /// Resolve a registry and schedule book into the eligible pool.
    ///
    /// Entries come out in content-id order — deterministic, and independent of registration order.
    pub fn resolve(registry: &ContentRegistry, book: &ScheduleBook) -> Self {
        let entries = registry
            .schedulable()
            .map(|(id, entry)| PoolEntry {
                content: id,
                kind: entry.kind(),
                schedule: book.get(id),
            })
            .collect();
        ContentPool { entries }
    }

    /// Every entry, in id order.
    pub fn entries(&self) -> &[PoolEntry] {
        &self.entries
    }

    /// How many pieces are in the pool.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Is the pool empty?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entries eligible at a progression, with their weights.
    pub fn eligible_at(&self, p: Progression) -> Vec<Candidate> {
        self.eligible_for(p, None)
    }

    /// The entries eligible at a progression **for a kind of scope**.
    ///
    /// Filtering by scope kind here is what keeps `unique` honest: a Biome cannot go in a room, so
    /// counting it among a room's available variety would inflate that room's target with content it
    /// could never use.
    ///
    /// Ignores [`Schedule::chance`], which is a roll rather than a property — the scheduler applies it,
    /// since only the scheduler has a stream to roll from.
    pub fn eligible_for(&self, p: Progression, scope: Option<NodeKind>) -> Vec<Candidate> {
        self.entries
            .iter()
            .filter_map(|e| {
                if let Some(kind) = scope {
                    if !e.schedule.scopes.allows(kind) {
                        return None;
                    }
                }
                let weight = e.schedule.weight_at(p);
                (weight > 0.0).then_some(Candidate {
                    content: e.content,
                    kind: e.kind,
                    weight,
                    chance: e.schedule.chance_at(p),
                })
            })
            .collect()
    }

    /// Content the world is *required* to contain, with its limit.
    ///
    /// L2 must place these; failing to is a failed generation rather than a sparse one.
    pub fn demands(&self) -> impl Iterator<Item = (ObjectId, WorldLimit)> + '_ {
        self.entries
            .iter()
            .filter(|e| e.schedule.world_limit.is_required())
            .map(|e| (e.content, e.schedule.world_limit))
    }

    /// The world-wide limit for a piece of content.
    pub fn world_limit(&self, content: ObjectId) -> WorldLimit {
        self.entries
            .iter()
            .find(|e| e.content == content)
            .map(|e| e.schedule.world_limit)
            .unwrap_or_default()
    }

    /// Only entries of a kind.
    pub fn of_kind(&self, kind: ContentKind) -> impl Iterator<Item = &PoolEntry> + '_ {
        self.entries.iter().filter(move |e| e.kind == kind)
    }
}

/// A piece of content eligible for a slot, with the weight it carries there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    /// Which content.
    pub content: ObjectId,
    /// What sort it is.
    pub kind: ContentKind,
    /// Relative preference among competing candidates.
    pub weight: f64,
    /// Probability it was offered here at all. `1.0` for content with no chance curve.
    pub chance: f64,
}

// ---------------------------------------------------------------------------------------------
// L1 — the plan
// ---------------------------------------------------------------------------------------------

/// One scope's target, with the candidates that may fill it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedSlot {
    /// The scope to fill.
    pub scope: Handle<Node>,
    /// How far through the world it sits.
    pub progression: Progression,
    /// How many things to aim for. **A target, not a guarantee** — L2 may place fewer.
    pub target: u32,
    /// What may fill it, in content-id order with weights.
    pub candidates: Vec<Candidate>,
    /// Why the target is what it is.
    pub reasoning: TargetReasoning,
}

/// **L1's output** — the authoritative plan L2 works from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SchedulePlan {
    slots: Vec<PlannedSlot>,
    demands: Vec<(ObjectId, WorldLimit)>,
}

impl SchedulePlan {
    /// Every slot, in graph-walk order.
    pub fn slots(&self) -> &[PlannedSlot] {
        &self.slots
    }

    /// How many slots were planned.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Were no slots planned?
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The plan for a scope.
    pub fn slot(&self, scope: Handle<Node>) -> Option<&PlannedSlot> {
        self.slots.iter().find(|s| s.scope == scope)
    }

    /// The total target across every slot.
    pub fn total_target(&self) -> u32 {
        self.slots.iter().map(|s| s.target).sum()
    }

    /// Slots that came out sparse — the ones a dev tuning content variety wants to see.
    pub fn scarce_slots(&self) -> impl Iterator<Item = &PlannedSlot> + '_ {
        self.slots
            .iter()
            .filter(|s| s.reasoning.outcome == TargetOutcome::Scarce)
    }

    /// Content the world is **required** to contain, carried through from L0.
    ///
    /// L1 cannot enforce a world-wide limit — it places nothing — so it records the demand and L2
    /// honours it. Without this hand-off, "exactly one final boss" would have nowhere to live: a
    /// per-slot count cannot express a world-wide fact.
    pub fn demands(&self) -> &[(ObjectId, WorldLimit)] {
        &self.demands
    }
}

/// Runs L1: turns a pool and a graph into per-scope targets.
///
/// Deterministic given the same graph, pool, rules and seed. Slot order follows the graph walk, and
/// each slot's jitter is drawn from a stream **forked on the scope's identity** rather than on its
/// position — so inserting a scope earlier in the world does not reshuffle every later slot's jitter.
pub struct Scheduler<'a> {
    graph: &'a NodeGraph,
    pool: &'a ContentPool,
    rules: Vec<SlotRule>,
}

impl<'a> Scheduler<'a> {
    /// A scheduler over a graph and pool.
    pub fn new(graph: &'a NodeGraph, pool: &'a ContentPool) -> Self {
        Scheduler {
            graph,
            pool,
            rules: Vec::new(),
        }
    }

    /// Add a rule for a scope kind.
    pub fn with_rule(mut self, rule: SlotRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// How far through the world a scope sits.
    ///
    /// Depth for now: the containing Reach's index over the Reach count. Generalised to a pluggable
    /// `ProgressionAxis` at M12.
    pub fn progression_of(&self, scope: Handle<Node>) -> Progression {
        let reaches: Vec<Handle<Node>> = self
            .graph
            .of_kind(NodeKind::Reach)
            .map(|(h, _)| h)
            .collect();
        if reaches.is_empty() {
            return Progression::START;
        }
        let Some(reach) = self.graph.scope_of(scope, NodeKind::Reach) else {
            return Progression::START;
        };
        match reaches.iter().position(|h| *h == reach) {
            // A single Reach is the whole world, so it sits at the start rather than dividing by zero.
            Some(_) if reaches.len() == 1 => Progression::START,
            Some(i) => Progression::new(i as f64 / (reaches.len() - 1) as f64),
            None => Progression::START,
        }
    }

    /// Produce the plan.
    pub fn plan(&self, rng: &Rng) -> SchedulePlan {
        let mut slots = Vec::new();
        for scope in self.graph.walk() {
            let Some(node) = self.graph.get(scope) else {
                continue;
            };
            let Some(rule) = self.rules.iter().find(|r| r.scope_kind == node.kind()) else {
                continue;
            };

            let progression = self.progression_of(scope);
            // Fork on identity, not index — see the struct docs.
            let slot_rng = rng.fork("slot").fork(&node.id().to_string());

            // Offer each candidate its chance roll, forked per content id so one piece's roll does
            // not depend on how many others happened to be considered first.
            let offer_rng = slot_rng.fork("offer");
            let candidates: Vec<Candidate> = self
                .pool
                .eligible_for(progression, Some(node.kind()))
                .into_iter()
                .filter(|c| {
                    c.chance >= 1.0 || offer_rng.fork(&c.content.to_string()).chance(c.chance)
                })
                .collect();

            // The slot's weight is the strongest candidate's: a slot backed by one strongly-favoured
            // piece should not be penalised for the others being weak here.
            let weight = candidates.iter().map(|c| c.weight).fold(0.0, math::max);

            let mut count_rng = slot_rng.fork("count");
            let reasoning =
                rule.count
                    .resolve(candidates.len() as u32, weight, progression, &mut count_rng);

            slots.push(PlannedSlot {
                scope,
                progression,
                target: reasoning.target,
                candidates,
                reasoning,
            });
        }
        SchedulePlan {
            slots,
            demands: self.pool.demands().collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Seed policy
// ---------------------------------------------------------------------------------------------

/// How much world to keep resolved around the frontier, and how long it should be.
///
/// ▶ **GAP:** `lookahead`/`lookbehind` only earn their keep once generation is genuinely on-demand
/// (M35). They are defined here because they belong to the project descriptor and therefore to the
/// fingerprint; the streaming policy that consumes them is M35's.
#[derive(Clone, Debug, PartialEq)]
pub struct SeedPolicy {
    /// Reaches to project ahead of the realized frontier.
    pub lookahead: u32,
    /// Reaches to keep behind it.
    pub lookbehind: u32,
    /// Target world length, in Reaches.
    pub length: AdaptiveRange,
}

impl Default for SeedPolicy {
    fn default() -> Self {
        SeedPolicy {
            lookahead: 2,
            lookbehind: 1,
            length: AdaptiveRange::new(3, 8),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------------------------

impl Serialize for Span {
    fn serialize(&self, w: &mut Writer) {
        w.f64(self.start);
        w.f64(self.end);
    }
}

impl Deserialize for Span {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Span::new(r.f64()?, r.f64()?))
    }
}

impl Serialize for Curve {
    fn serialize(&self, w: &mut Writer) {
        w.len(self.points.len());
        for (x, y) in &self.points {
            w.f64(*x);
            w.f64(*y);
        }
    }
}

impl Deserialize for Curve {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let n = r.u32()? as usize;
        let mut points = Vec::with_capacity(n.min(r.remaining()));
        for _ in 0..n {
            points.push((r.f64()?, r.f64()?));
        }
        if points.is_empty() {
            return Err(SerError::InvalidValue(
                "a curve needs at least one keyframe",
            ));
        }
        Ok(Curve::from_points(points))
    }
}

impl Serialize for AdaptiveRange {
    fn serialize(&self, w: &mut Writer) {
        w.u32(self.soft_min);
        w.u32(self.hard_max);
        w.f64(self.repeat_tol);
        w.u32(self.jitter);
    }
}

impl Deserialize for AdaptiveRange {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(AdaptiveRange {
            soft_min: r.u32()?,
            hard_max: r.u32()?,
            repeat_tol: r.f64()?,
            jitter: r.u32()?,
        })
    }
}

impl Serialize for ScopeFilter {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.0);
    }
}

impl Deserialize for ScopeFilter {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let bits = r.u8()?;
        if bits & !ScopeFilter::ANY.0 != 0 {
            return Err(SerError::InvalidValue(
                "ScopeFilter has bits for no NodeKind",
            ));
        }
        Ok(ScopeFilter(bits))
    }
}

impl Serialize for WorldLimit {
    fn serialize(&self, w: &mut Writer) {
        w.u32(self.min);
        w.write(&self.max);
    }
}

impl Deserialize for WorldLimit {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let min = r.u32()?;
        let max: Option<u32> = r.read()?;
        if max.is_some_and(|m| m < min) {
            return Err(SerError::InvalidValue("WorldLimit max is below min"));
        }
        Ok(WorldLimit { min, max })
    }
}

impl Serialize for CountRule {
    fn serialize(&self, w: &mut Writer) {
        match self {
            CountRule::Fixed(n) => {
                w.u8(0);
                w.u32(*n);
            }
            CountRule::Range { min, max } => {
                w.u8(1);
                w.u32(*min);
                w.u32(*max);
            }
            CountRule::Curve(c) => {
                w.u8(2);
                w.write(c);
            }
            CountRule::Adaptive(r) => {
                w.u8(3);
                w.write(r);
            }
        }
    }
}

impl Deserialize for CountRule {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => CountRule::Fixed(r.u32()?),
            1 => CountRule::range(r.u32()?, r.u32()?),
            2 => CountRule::Curve(r.read()?),
            3 => CountRule::Adaptive(r.read()?),
            _ => return Err(SerError::InvalidValue("unknown CountRule tag")),
        })
    }
}

impl Serialize for Schedule {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.span);
        w.write(&self.weight);
        w.write(&self.chance);
        w.write(&self.scopes);
        w.write(&self.world_limit);
    }
}

impl Deserialize for Schedule {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Schedule {
            span: r.read()?,
            weight: r.read()?,
            chance: r.read()?,
            scopes: r.read()?,
            world_limit: r.read()?,
        })
    }
}

impl Serialize for ScheduleBook {
    fn serialize(&self, w: &mut Writer) {
        w.len(self.schedules.len());
        for (id, schedule) in &self.schedules {
            w.write(id);
            w.write(schedule);
        }
    }
}

impl Deserialize for ScheduleBook {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let n = r.u32()? as usize;
        let mut schedules = BTreeMap::new();
        for _ in 0..n {
            let id: ObjectId = r.read()?;
            let schedule: Schedule = r.read()?;
            schedules.insert(id, schedule);
        }
        Ok(ScheduleBook { schedules })
    }
}

impl Serialize for SeedPolicy {
    fn serialize(&self, w: &mut Writer) {
        w.u32(self.lookahead);
        w.u32(self.lookbehind);
        w.write(&self.length);
    }
}

impl Deserialize for SeedPolicy {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(SeedPolicy {
            lookahead: r.u32()?,
            lookbehind: r.u32()?,
            length: r.read()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::{from_bytes, to_bytes};

    #[test]
    fn curves_interpolate_and_clamp() {
        let c = Curve::from_points([(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)]);
        assert_eq!(c.eval(Progression::new(0.0)), 0.0);
        assert_eq!(c.eval(Progression::new(0.25)), 0.5);
        assert_eq!(c.eval(Progression::new(0.5)), 1.0);
        assert_eq!(c.eval(Progression::new(0.75)), 0.5);
        // Outside the keyed range it holds, rather than extrapolating into nonsense.
        assert_eq!(
            Curve::from_points([(0.3, 2.0), (0.7, 4.0)]).eval(Progression::START),
            2.0
        );
        assert_eq!(
            Curve::from_points([(0.3, 2.0), (0.7, 4.0)]).eval(Progression::END),
            4.0
        );
        assert_eq!(Curve::constant(0.7).eval(Progression::new(0.42)), 0.7);
        assert_eq!(Curve::ramp(0.0, 1.0).eval(Progression::new(0.3)), 0.3);
    }

    #[test]
    fn curves_are_canonical_regardless_of_key_order() {
        let a = Curve::from_points([(1.0, 5.0), (0.0, 1.0), (0.5, 3.0)]);
        let b = Curve::from_points([(0.0, 1.0), (0.5, 3.0), (1.0, 5.0)]);
        assert_eq!(a, b);
        assert_eq!(a.eval(Progression::new(0.25)), 2.0);
    }

    #[test]
    fn spans_gate_eligibility() {
        let late = Span::from(0.6);
        assert!(!late.contains(Progression::new(0.5)));
        assert!(
            late.contains(Progression::new(0.6)),
            "inclusive at the start"
        );
        assert!(late.contains(Progression::END));
        // Reversed inputs are sorted rather than producing an empty span.
        assert_eq!(Span::new(0.8, 0.2), Span::new(0.2, 0.8));
        assert!(Span::ALWAYS.contains(Progression::START));
    }

    #[test]
    fn adaptive_range_tracks_available_variety() {
        let range = AdaptiveRange::new(3, 6).with_repeat_tol(1.5);
        let mut rng = Rng::new(1);

        // Abundant: 10 unique × 1.5 = 15, over the ceiling.
        let abundant = range.resolve(10, 1.0, &mut rng);
        assert_eq!(abundant.outcome, TargetOutcome::Abundant);
        assert_eq!(abundant.target, 6, "never exceeds hard_max");

        // Moderate: 3 unique × 1.5 = 4, between soft_min and hard_max.
        let moderate = range.resolve(3, 1.0, &mut rng);
        assert_eq!(moderate.outcome, TargetOutcome::Moderate);
        assert_eq!(moderate.target, 4);

        // Scarce: 1 unique × 1.5 = 1, below soft_min — and it stays there.
        let scarce = range.resolve(1, 1.0, &mut rng);
        assert_eq!(scarce.outcome, TargetOutcome::Scarce);
        assert_eq!(
            scarce.target, 1,
            "soft_min is a preference: a scarce slot reads sparse rather than being padded"
        );
    }

    #[test]
    fn weight_scales_the_target_and_zero_weight_empties_a_slot() {
        let range = AdaptiveRange::new(0, 10).with_repeat_tol(1.0);
        let mut rng = Rng::new(1);
        assert_eq!(range.resolve(8, 1.0, &mut rng).target, 8);
        assert_eq!(range.resolve(8, 0.5, &mut rng).target, 4);
        assert_eq!(range.resolve(8, 0.0, &mut rng).target, 0);
        // Weights above 1 are clamped rather than inflating the count.
        assert_eq!(range.resolve(8, 5.0, &mut rng).target, 8);
    }

    #[test]
    fn repeat_tolerance_controls_density() {
        let mut rng = Rng::new(1);
        let unique = 4;
        // No reuse at all: one of each.
        assert_eq!(
            AdaptiveRange::new(0, 20)
                .with_repeat_tol(1.0)
                .resolve(unique, 1.0, &mut rng)
                .target,
            4
        );
        // Tolerating triple reuse packs the room denser.
        assert_eq!(
            AdaptiveRange::new(0, 20)
                .with_repeat_tol(3.0)
                .resolve(unique, 1.0, &mut rng)
                .target,
            12
        );
    }

    #[test]
    fn jitter_is_reproducible_and_respects_the_ceiling() {
        let range = AdaptiveRange::new(0, 5).with_repeat_tol(1.0).with_jitter(2);
        // The same stream gives the same wobble.
        let mut a = Rng::new(99);
        let mut b = Rng::new(99);
        assert_eq!(range.resolve(3, 1.0, &mut a), range.resolve(3, 1.0, &mut b));

        // Jitter never breaches hard_max, and never goes negative.
        let mut rng = Rng::new(7);
        for _ in 0..500 {
            let r = range.resolve(5, 1.0, &mut rng);
            assert!(r.target <= 5, "jitter must not exceed hard_max");
        }
        let mut rng = Rng::new(7);
        for _ in 0..500 {
            assert!(range.resolve(0, 1.0, &mut rng).target <= 2);
        }
    }

    #[test]
    fn reasoning_explains_the_number() {
        let mut rng = Rng::new(1);
        let r = AdaptiveRange::new(3, 6)
            .with_repeat_tol(1.5)
            .resolve(2, 0.5, &mut rng);
        assert_eq!(r.unique, 2);
        assert_eq!(r.supported, 1, "floor(2 × 1.5 × 0.5)");
        assert_eq!(r.outcome, TargetOutcome::Scarce);
        let text = r.to_string();
        assert!(text.contains("scarce"), "{text}");
        assert!(text.contains("2 unique"), "{text}");
    }

    #[test]
    fn schedule_weight_is_zero_outside_its_span() {
        let s = Schedule::during(Span::from(0.5)).weighted(Curve::constant(0.8));
        assert_eq!(s.weight_at(Progression::new(0.4)), 0.0);
        assert_eq!(s.weight_at(Progression::new(0.6)), 0.8);
        assert!(!s.is_eligible(Progression::START));
        assert!(s.is_eligible(Progression::END));
        // Negative curve values are clamped — a weight is never below zero.
        let negative = Schedule::always().weighted(Curve::constant(-1.0));
        assert_eq!(negative.weight_at(Progression::START), 0.0);
    }

    #[test]
    fn serialization_round_trips() {
        let s = Schedule::during(Span::new(0.2, 0.9))
            .weighted(Curve::from_points([(0.0, 0.1), (1.0, 0.9)]));
        assert_eq!(from_bytes::<Schedule>(&to_bytes(&s)).unwrap(), s);

        let r = AdaptiveRange::new(2, 7)
            .with_repeat_tol(1.25)
            .with_jitter(3);
        assert_eq!(from_bytes::<AdaptiveRange>(&to_bytes(&r)).unwrap(), r);

        let mut book = ScheduleBook::new();
        book.set(ObjectId::derived("actor", "door"), s);
        assert_eq!(from_bytes::<ScheduleBook>(&to_bytes(&book)).unwrap(), book);

        let policy = SeedPolicy::default();
        assert_eq!(
            from_bytes::<SeedPolicy>(&to_bytes(&policy)).unwrap(),
            policy
        );

        // A curve with no keys is not a curve.
        let mut w = Writer::with_envelope();
        w.u32(0);
        assert!(from_bytes::<Curve>(&w.finish()).is_err());
    }
}
