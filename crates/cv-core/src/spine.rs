//! **Spine Templates** — opt-in macro-structure: a guaranteed sequence of node slots with free-form
//! connective tissue between them.
//!
//! # A spine is a guarantee, not a bias
//!
//! A structural pattern that only *sometimes* holds is worse than none. A player who meets a
//! boss-then-treasury rhythm three Reaches running and then does not has found a bug, not variety. So
//! when a dev registers a spine they are stating a contract, and if it cannot be delivered they want
//! to know at authoring time — not to receive a world quietly missing a room.
//!
//! # Guarantee and freedom apply to different things
//!
//! ```text
//! start ──<anything>──▶ precursor ──<anything>──▶ capstone
//!   └ GUARANTEED           └ GUARANTEED             └ GUARANTEED     (slots)
//!         └──── free-form, dialed, branchable ────┘                 (segments)
//! ```
//!
//! [`SpineSlot`]s exist, in order, with their properties. [`SpineSegment`]s — the `<anything>` — are
//! generated: their length and character are dialed. Branching off a slot stays free; the guarantee is
//! about a slot's *existence and properties*, not about the world being a corridor.
//!
//! # Strictness is a spectrum, but a *declared* one
//!
//! > **Every relaxation is declared. Nothing is loose by accident.**
//!
//! "This slot is optional" is fine — the dev chose it and can plan around it. "This slot is required
//! but sometimes missing" is the bug. The difference is not how often it holds; it is whether the dev
//! **said so**. Hence three tiers ([`Strictness`]) and an [`adherence`](SpineTemplate::adherence) dial
//! that **modulates the soft tiers only and can never touch a `Required`**.
//!
//! # Still opt-in
//!
//! Most games register none, and registering none must reproduce the free-form M09 behaviour
//! *exactly*. `prime` registers none deliberately: MP1 has no single spine, and that preset is the
//! proof the default is genuine rather than nominal.

use crate::content::ContentRegistry;
use crate::mission::{MissionEdge, MissionGraph, Rule};
use crate::node::{Node, NodeGraph, NodeKind};
use crate::object::{Object, ObjectId};
use crate::schedule::{AdaptiveRange, Curve, Progression};
use crate::Handle;
use cv_determinism::{math, Rng};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// ---------------------------------------------------------------------------------------------
// Strictness
// ---------------------------------------------------------------------------------------------

/// How binding a spine, slot, or requirement is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strictness {
    /// Must hold; if it cannot, generation **fails** with a diagnostic. The default, because an
    /// unexpected guarantee is merely restrictive while an unexpected relaxation is a bug in the field.
    #[default]
    Required,
    /// Strongly biased; may be relaxed when infeasible — and is **reported** when it is.
    Preferred,
    /// The generator decides freely; absence is expected, not a failure.
    Optional,
}

impl Strictness {
    /// Is this binding?
    pub fn is_required(self) -> bool {
        self == Strictness::Required
    }

    /// Minimum `adherence` at which a slot of this tier is kept.
    ///
    /// `Required` returns `0.0` — **the dial cannot reach it**, which is the property that makes a
    /// scalar safe here where the original design's single adherence dial was not.
    ///
    /// ▶ The two soft thresholds are a first cut; tuning them wants real worlds, not a guess.
    pub fn adherence_threshold(self) -> f64 {
        match self {
            Strictness::Required => 0.0,
            Strictness::Preferred => 0.25,
            Strictness::Optional => 0.75,
        }
    }

    fn tag(self) -> u8 {
        match self {
            Strictness::Required => 0,
            Strictness::Preferred => 1,
            Strictness::Optional => 2,
        }
    }
}

impl fmt::Display for Strictness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Strictness::Required => "required",
            Strictness::Preferred => "preferred",
            Strictness::Optional => "optional",
        })
    }
}

/// What a slot means **to the mission graph** — the only two positions the core itself has an opinion
/// about.
///
/// # Why this is not a vocabulary of level-design words
///
/// A slot's *name* is the dev's word: `"capstone"`, `"treasury"`, `"antechamber"`, `"the bit with the
/// dogs"`. The core neither knows nor cares what those mean, and a role variant borrowing one of them
/// would be pretending to a significance it does not have — worse, it would suggest the core treats
/// `"terminal"` specially when the string is just a label the dev chose.
///
/// So the role enum names only what the core can *act on*: [`MissionGraph::start`] and
/// [`MissionGraph::goal`]. Both do something — a `Start` slot becomes the graph's start, a `Goal` slot
/// becomes its goal, which is what M10's un-softlockable guarantee is stated against. A role that
/// merely described would be a comment with a type.
///
/// **Role and shape are orthogonal.** `Goal` says *"the run ends here"*, never *"nothing leads
/// onward"* — a dungeon boss chamber is a goal you walk back out of. Say which you mean with
/// [`min_degree`](SpineSlot::min_degree) / [`max_degree`](SpineSlot::max_degree).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlotRole {
    /// The run begins here — this slot becomes [`MissionGraph::start`].
    ///
    /// A world always has a start, spine or not; declaring one here just moves it onto a guaranteed
    /// slot instead of wherever the caller happened to point.
    Start,
    /// Neither endpoint. The default, and what almost every slot is.
    #[default]
    Interior,
    /// The run is complete here — this slot becomes [`MissionGraph::goal`].
    Goal,
}

// ---------------------------------------------------------------------------------------------
// Symbolic unlock references
// ---------------------------------------------------------------------------------------------

/// A unlock named either directly or **by whichever slot granted it**.
///
/// [`UnlockRef::GrantedBy`] is what makes the Zelda-dungeon pattern expressible at all: a
/// generated dungeon cannot *name* the unlock it hands out, so a spine says "gate this segment on
/// whatever the precursor granted, and make the boss require it too". Both references resolve to the
/// same choice at instantiation, so the theme holds however the generator resolves it.
///
/// Without this, the pattern could only be written by hard-coding a unlock — defeating the point
/// of generating.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnlockRef {
    /// This exact unlock.
    Explicit(ObjectId),
    /// Whatever the named slot ended up granting.
    GrantedBy(String),
}

/// What a slot's content may grant. The generator picks; the dev constrains.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GrantSpec {
    /// Candidate unlocks. The instantiator chooses one deterministically.
    pub any_of: Vec<ObjectId>,
}

impl GrantSpec {
    /// Any one of these unlocks.
    pub fn any_of(caps: impl IntoIterator<Item = ObjectId>) -> Self {
        GrantSpec {
            any_of: caps.into_iter().collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Slots and segments
// ---------------------------------------------------------------------------------------------

/// What a slot demands of its **topology** — consumed by L2.
///
/// Grouped rather than flattened onto [`SpineSlot`] because these are the constraints one *layer*
/// reads, and because a slot that says nothing about shape should cost nothing to write or to read.
/// The builder methods stay on `SpineSlot`, so authoring is unaffected by the grouping.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotShape {
    /// Minimum connections this scope must have ("four entrances").
    ///
    /// A **floor**: free-form growth and `cycle_density` may add more, and typically do. Satisfied
    /// before the dials run, so a dev at zero cycles still gets their multi-entrance arena.
    pub min_degree: Option<u32>,
    /// Maximum connections this scope may have. `Some(1)` is a dead end.
    ///
    /// A **ceiling**, and the harder of the two, because every pass *after* the spine could violate it.
    /// Recorded as a cap on the mission graph and enforced inside
    /// [`MissionGraph::add_edge`](crate::mission::MissionGraph::add_edge), so it holds against
    /// `cycle_density` at full tilt rather than merely preceding it.
    pub max_degree: Option<u32>,
    /// Slots this must be directly connected to ("the treasury is an exit of the capstone").
    pub adjacent_to: Vec<String>,
}

/// What a slot demands of its **contents** — consumed by L0/L1 and the solver.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotContents {
    /// Content that must be placed here.
    pub must_contain: Vec<ObjectId>,
    /// Content preferred here — the soft counterpart.
    pub prefer_contain: Vec<ObjectId>,
    /// State the player must hold to reach this slot.
    pub requires: Option<UnlockRef>,
    /// What the content placed here grants.
    pub grants: Option<GrantSpec>,
    /// **The generator places nothing here.** No scheduled content, no item locations, no gates
    /// inside — the scope's interior belongs entirely to the host.
    ///
    /// This is *empty as generation sees it*, not empty as the player sees it. A room declared empty
    /// is typically full of the host's own fixed furniture — a shop, a save point, a hub — and the
    /// whole point is that none of it is the generator's business. Because nothing there varies, it is
    /// also the one kind of room a dev can guarantee looks identical every run.
    ///
    /// A host finds it by slot name through [`WorldDescriptor::spine_slot`](crate::WorldDescriptor::spine_slot)
    /// and furnishes it itself.
    pub empty: bool,
}

/// One guaranteed node in the sequence.
///
/// # Why the fields are grouped
///
/// A slot accumulates constraints from several directions — topology, contents, and (later) pacing and
/// spatial hints. Flattening them all would produce a struct of twenty-odd optional fields where
/// nothing indicates *which layer reads what*, and where every addition lengthens one undifferentiated
/// list.
///
/// So constraints live in small groups named after the layer that consumes them ([`SlotShape`],
/// [`SlotContents`]), each independently defaultable — a dev pays only for the axes they use.
/// **The builder surface stays flat**: `SpineSlot::new("x").min_degree(3).empty()` works regardless of
/// which group a field lives in. The grouping is for the type and for readers, not for authors.
#[derive(Clone, Debug, PartialEq)]
pub struct SpineSlot {
    /// Referenceable name — `"capstone"`, `"treasury"`, whatever the dev calls it. The core attaches no
    /// meaning to it; see [`SlotRole`] for the two positions it *does* have an opinion about.
    pub name: String,
    /// A registered `SlotPurpose` resource (boss-arena, treasury, …). Registered rather than a free
    /// string so a rename is a compile error, not a spine that quietly stops matching.
    pub purpose: Option<ObjectId>,
    /// Structural role.
    pub role: SlotRole,
    /// Overrides the template's strictness for this slot.
    pub strictness: Option<Strictness>,
    /// Overrides the template's adherence for this slot.
    pub adherence: Option<f64>,
    /// Topology demands — what this scope connects to, and how much.
    pub shape: SlotShape,
    /// Content demands — what goes in it, or that nothing does.
    pub contents: SlotContents,
    // ▶ Future groups land here rather than lengthening the list above: `pacing` (sphere bounds, gate
    // budget) and `space` (volume and theming hints for L3/L4).
}

impl SpineSlot {
    /// A plain interior slot, demanding nothing.
    pub fn new(name: impl Into<String>) -> Self {
        SpineSlot {
            name: name.into(),
            purpose: None,
            role: SlotRole::Interior,
            strictness: None,
            adherence: None,
            shape: SlotShape::default(),
            contents: SlotContents::default(),
        }
    }

    /// Set the structural role.
    pub fn role(mut self, role: SlotRole) -> Self {
        self.role = role;
        self
    }

    /// Set the purpose resource.
    pub fn purpose(mut self, purpose: ObjectId) -> Self {
        self.purpose = Some(purpose);
        self
    }

    /// Override strictness for this slot.
    pub fn strictness(mut self, s: Strictness) -> Self {
        self.strictness = Some(s);
        self
    }

    /// Require content here.
    pub fn must_contain(mut self, content: impl IntoIterator<Item = ObjectId>) -> Self {
        self.contents.must_contain.extend(content);
        self
    }

    /// Prefer content here — the soft counterpart of [`must_contain`](Self::must_contain).
    pub fn prefer_contain(mut self, content: impl IntoIterator<Item = ObjectId>) -> Self {
        self.contents.prefer_contain.extend(content);
        self
    }

    /// Require state to reach here.
    pub fn requires(mut self, requirement: UnlockRef) -> Self {
        self.contents.requires = Some(requirement);
        self
    }

    /// Declare that this slot grants a unlock.
    pub fn grants(mut self, spec: GrantSpec) -> Self {
        self.contents.grants = Some(spec);
        self
    }

    /// Require a minimum number of connections.
    pub fn min_degree(mut self, degree: u32) -> Self {
        self.shape.min_degree = Some(degree);
        self
    }

    /// Allow at most this many connections.
    pub fn max_degree(mut self, degree: u32) -> Self {
        self.shape.max_degree = Some(degree);
        self
    }

    /// A dead end: one way in, which is also the way out.
    ///
    /// Reached from exactly one room, with nothing beyond it, and the cap holds even at
    /// `cycle_density = 1.0`.
    pub fn dead_end(self) -> Self {
        self.max_degree(1)
    }

    /// The generator places nothing here — see [`SlotContents::empty`].
    ///
    /// Pairs naturally with [`dead_end`](Self::dead_end) for a room the host owns outright:
    ///
    /// ```ignore
    /// SpineSlot::new("treasury").adjacent_to("capstone").dead_end().empty()
    /// ```
    pub fn empty(mut self) -> Self {
        self.contents.empty = true;
        self
    }

    /// Require a direct connection to another slot.
    pub fn adjacent_to(mut self, slot: impl Into<String>) -> Self {
        self.shape.adjacent_to.push(slot.into());
        self
    }

    /// The strictness in force, given the template default.
    pub fn effective_strictness(&self, template: Strictness) -> Strictness {
        self.strictness.unwrap_or(template)
    }
}

/// The free-form stretch between two consecutive slots.
///
/// # What a segment guarantees, and what it leaves open
///
/// A segment promises exactly one thing: **a route exists from `from` to `to`**. It does *not* promise
/// that route is the only one, and it does not describe the shape of what fills the gap.
///
/// That distinction is the whole point of the slot/segment split. Slots are where a dev takes a
/// decision away from the generator; segments are where they hand one back. So the interior of a
/// segment may **branch and reconverge** freely — the scopes inside it are never degree-capped, and
/// `cycle_density`, the spatial adjacency, and every later pass are all entitled to add connections
/// through it. A dev who wants "*something* between the boss and the exit, I don't care what" writes
/// [`free`](Self::free) and is answered by the algorithm rather than by their own diagram.
///
/// ▶ **v0.1 limitation.** The *shape* of the interior is decided by the dials and the spatial graph,
/// not stated by the dev. There is no way yet to say "branch into exactly two wings here and rejoin";
/// that wants the parallel-slot work recorded in the depth-ladder design notes. What exists today is
/// the honest version of "anything can go here" — not a promise of any particular topology.
#[derive(Clone, Debug, PartialEq)]
pub struct SpineSegment {
    /// The slot it leaves.
    pub from: String,
    /// The slot it reaches.
    pub to: String,
    /// How many scopes of connective tissue. `0..=0` means a direct connection.
    pub length: AdaptiveRange,
    // ▶ **M09.** A per-segment dial override belongs here — *"free-form here, tight there"* — but
    // as a **user-authored dial** on the slot, not a core struct. The pre-v0.1 `LinearityOverride`
    // that used to sit here carried `progression_locality`, which the design refuses outright.
    /// What the path through here requires.
    pub gated_by: Option<UnlockRef>,
}

impl SpineSegment {
    /// A segment of the given length between two slots.
    pub fn new(from: impl Into<String>, to: impl Into<String>, length: AdaptiveRange) -> Self {
        SpineSegment {
            from: from.into(),
            to: to.into(),
            length,
            gated_by: None,
        }
    }

    /// A direct connection — no intervening scopes.
    pub fn direct(from: impl Into<String>, to: impl Into<String>) -> Self {
        SpineSegment::new(from, to, AdaptiveRange::new(0, 0))
    }

    /// **Anything may go here — the algorithm decides how much, and what shape.**
    ///
    /// The explicit way to hand a stretch of the world back to the generator: no declared length, no
    /// declared topology, just "get me from `from` to `to`". Whatever scopes the instance has spare
    /// are available to it, and the interior may branch and reconverge as the dials and the spatial
    /// graph see fit.
    ///
    /// Contrast [`new`](Self::new), which bounds the length, and [`direct`](Self::direct), which
    /// forbids any. All three say the same *kind* of thing — how much latitude the generator has —
    /// which is why there is one mechanism rather than three.
    pub fn free(from: impl Into<String>, to: impl Into<String>) -> Self {
        SpineSegment::new(from, to, AdaptiveRange::new(0, Self::UNBOUNDED))
    }

    /// The ceiling [`free`](Self::free) uses — "as much as the instance can spare".
    ///
    /// Not `u32::MAX`: headroom is summed across segments to apportion surplus, and a value that
    /// large would overflow that sum for no benefit. This is far past any plausible scope count while
    /// staying comfortably summable.
    pub const UNBOUNDED: u32 = u32::MAX / 1024;

    /// Is this segment's length unconstrained?
    pub fn is_free(&self) -> bool {
        self.length.hard_max >= Self::UNBOUNDED
    }

    /// Gate the path through this segment.
    pub fn gated_by(mut self, requirement: UnlockRef) -> Self {
        self.gated_by = Some(requirement);
        self
    }

    /// The fewest scopes this segment can consume.
    pub fn min_length(&self) -> u32 {
        self.length.soft_min.min(self.length.hard_max)
    }
}

// ---------------------------------------------------------------------------------------------
// Coverage
// ---------------------------------------------------------------------------------------------

/// Which instances of the applicable scope receive the spine.
///
/// Deliberately a **pattern, not a probability**. "Boss Reaches every third" is a design; a 33% chance
/// is a lottery, and a lottery is exactly the unpredictability a guaranteed spine exists to avoid.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Coverage {
    /// Every instance.
    #[default]
    All,
    /// Every nth instance, starting from the first.
    Every(u32),
    /// A fraction, spread **evenly** by accumulation rather than sampled — so 0.5 gives alternating
    /// instances, not a coin flip per instance.
    Fraction(Curve),
    /// Exactly these instance indices.
    Indices(Vec<u32>),
}

impl Coverage {
    /// Which of `total` instances receive the spine, in ascending order.
    pub fn selected(&self, total: usize) -> Vec<usize> {
        match self {
            Coverage::All => (0..total).collect(),
            Coverage::Every(n) => {
                let step = (*n).max(1) as usize;
                (0..total).step_by(step).collect()
            }
            Coverage::Fraction(curve) => {
                // Evenly spread by accumulating the fraction and emitting when it crosses an integer.
                // Deterministic and gap-free — the standard way to distribute a ratio without a
                // lottery.
                let mut out = Vec::new();
                let mut accumulator = 0.0f64;
                for i in 0..total {
                    let p = if total <= 1 {
                        Progression::START
                    } else {
                        Progression::new(i as f64 / (total - 1) as f64)
                    };
                    accumulator += math::saturate(curve.eval(p));
                    if accumulator >= 1.0 {
                        accumulator -= 1.0;
                        out.push(i);
                    }
                }
                out
            }
            Coverage::Indices(indices) => {
                let mut out: Vec<usize> = indices
                    .iter()
                    .map(|i| *i as usize)
                    .filter(|i| *i < total)
                    .collect();
                out.sort_unstable();
                out.dedup();
                out
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The template
// ---------------------------------------------------------------------------------------------

/// An opt-in macro-structure constraint on L2's topology.
#[derive(Clone, Debug, PartialEq)]
pub struct SpineTemplate {
    /// This template's registered content id.
    pub id: ObjectId,
    /// Ordered guaranteed slots.
    pub slots: Vec<SpineSlot>,
    /// Connective tissue between consecutive slots.
    pub segments: Vec<SpineSegment>,
    /// Which scope kind an instance covers; the spine repeats per instance.
    pub applies_to: NodeKind,
    /// Template-wide strictness default.
    pub strictness: Strictness,
    /// Modulates the **soft tiers only**. `Required` is dial-immune.
    pub adherence: f64,
    /// Which instances receive it.
    pub coverage: Coverage,
}

impl SpineTemplate {
    /// A template over a scope kind.
    pub fn new(id: ObjectId, applies_to: NodeKind) -> Self {
        SpineTemplate {
            id,
            slots: Vec::new(),
            segments: Vec::new(),
            applies_to,
            strictness: Strictness::Required,
            adherence: 1.0,
            coverage: Coverage::All,
        }
    }

    /// Append a slot.
    pub fn slot(mut self, slot: SpineSlot) -> Self {
        self.slots.push(slot);
        self
    }

    /// Append a segment.
    pub fn segment(mut self, segment: SpineSegment) -> Self {
        self.segments.push(segment);
        self
    }

    /// Set the template-wide strictness.
    pub fn strictness(mut self, s: Strictness) -> Self {
        self.strictness = s;
        self
    }

    /// Set the adherence dial.
    pub fn adherence(mut self, a: f64) -> Self {
        self.adherence = math::saturate(a);
        self
    }

    /// Set the coverage pattern.
    pub fn coverage(mut self, c: Coverage) -> Self {
        self.coverage = c;
        self
    }

    /// Look up a slot by name.
    pub fn slot_named(&self, name: &str) -> Option<&SpineSlot> {
        self.slots.iter().find(|s| s.name == name)
    }

    /// Is a slot kept, given the dials?
    ///
    /// `Required` ignores adherence entirely — that is the whole safety property of the dial.
    pub fn keeps(&self, slot: &SpineSlot) -> bool {
        let strictness = slot.effective_strictness(self.strictness);
        if strictness.is_required() {
            return true;
        }
        let adherence = slot.adherence.unwrap_or(self.adherence);
        adherence >= strictness.adherence_threshold()
    }

    /// The slots that survive the dials, in order.
    pub fn kept_slots(&self) -> Vec<&SpineSlot> {
        self.slots.iter().filter(|s| self.keeps(s)).collect()
    }

    /// The fewest scopes an instance needs.
    ///
    /// **A spine raises a floor on the scope budget rather than competing with it** — L1's
    /// `AdaptiveRange` and a spine are talking about the same number, so the spine informs it.
    pub fn required_minimum(&self) -> u32 {
        let slots = self
            .slots
            .iter()
            .filter(|s| s.effective_strictness(self.strictness).is_required())
            .count() as u32;
        let segments: u32 = self
            .segments
            .iter()
            .filter(|seg| {
                // Only segments between two Required slots contribute a hard minimum.
                [&seg.from, &seg.to].iter().all(|name| {
                    self.slot_named(name)
                        .map(|s| s.effective_strictness(self.strictness).is_required())
                        .unwrap_or(false)
                })
            })
            .map(|seg| seg.min_length())
            .sum();
        slots + segments
    }

    /// How many connective scopes each gap between consecutive kept slots gets, given `capacity`
    /// scopes to work with.
    ///
    /// Surplus beyond the guaranteed floor is spread across the segments **proportionally to their
    /// declared headroom** (`hard_max - min_length`) — a segment that asked to be able to run long
    /// gets more of it. The remainder goes to the largest fractional shares, ties to the earlier
    /// segment.
    ///
    /// Deliberately **integer arithmetic end to end**: this decides where the capstone physically
    /// lands, so float rounding differing between targets would mean two players getting differently
    /// shaped worlds from one seed.
    pub fn segment_lengths(&self, kept: &[&SpineSlot], capacity: usize) -> Vec<u32> {
        if kept.len() < 2 {
            return Vec::new();
        }
        let gaps: Vec<Option<&SpineSegment>> = kept
            .windows(2)
            .map(|pair| {
                self.segments
                    .iter()
                    .find(|s| s.from == pair[0].name && s.to == pair[1].name)
            })
            .collect();

        let mut lengths: Vec<u32> = gaps
            .iter()
            .map(|g| g.map(|s| s.min_length()).unwrap_or(0))
            .collect();

        // What is left after the slots themselves and every segment's floor.
        let floor: u32 = lengths.iter().sum::<u32>() + kept.len() as u32;
        let mut surplus = (capacity as u32).saturating_sub(floor);
        if surplus == 0 {
            return lengths;
        }

        let headroom: Vec<u32> = gaps
            .iter()
            .zip(&lengths)
            .map(|(g, min)| {
                g.map(|s| s.length.hard_max.saturating_sub(*min))
                    .unwrap_or(0)
            })
            .collect();
        // Summed as `u64`: a `free` segment declares an enormous ceiling, and several of them together
        // would wrap a `u32` — silently, and into a *smaller* number, which would look like a spine
        // that mysteriously stopped using its budget.
        let total: u64 = headroom.iter().map(|h| u64::from(*h)).sum();
        if total == 0 {
            return lengths;
        }
        surplus = surplus.min(total.min(u64::from(u32::MAX)) as u32);

        // Proportional shares, then the remainder by largest fractional part.
        let mut remainders: Vec<(u64, usize)> = Vec::with_capacity(headroom.len());
        let mut handed_out = 0u32;
        for (i, h) in headroom.iter().enumerate() {
            let exact = u64::from(surplus) * u64::from(*h);
            let share = (exact / total) as u32;
            lengths[i] += share;
            handed_out += share;
            remainders.push((exact % total, i));
        }
        // Descending remainder, ascending index — a total order, so the outcome is one value.
        remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, i) in remainders.iter().take((surplus - handed_out) as usize) {
            lengths[*i] += 1;
        }
        lengths
    }
}

// ---------------------------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------------------------

/// A `Required` condition that cannot hold — generation must not proceed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpineError {
    /// Not enough scopes for the guaranteed slots and segments.
    BudgetTooSmall { needed: u32, available: u32 },
    /// A slot requires content nothing registered can satisfy.
    ContentUnavailable { slot: String, content: ObjectId },
    /// A reference names a slot that does not exist.
    UnknownSlot { referenced_by: String, name: String },
    /// Two slots share a name, so references are ambiguous.
    DuplicateSlot { name: String },
    /// A slot requires a unlock granted only by a *later* slot — an unsatisfiable order.
    GrantOrderViolation { slot: String, granted_by: String },
    /// A slot needs more connections than the instance can supply.
    DegreeInfeasible {
        slot: String,
        needed: u32,
        available: u32,
    },
    /// A slot's `max_degree` cannot accommodate what the same slot also demands.
    ///
    /// Usually a dead end that is also asked to touch two other slots. One requirement contradicts the
    /// other, and picking a winner silently would mean quietly dropping something the dev wrote.
    DegreeContradiction {
        /// The slot.
        slot: String,
        /// Its ceiling.
        max: u32,
        /// The connections it also requires, counting the route in.
        required: u32,
        /// What forces that number.
        because: String,
    },
    /// A slot declared `empty` is also told to hold something.
    EmptyContradiction {
        /// The slot.
        slot: String,
        /// The field that contradicts it.
        because: String,
    },
    /// The template declares no slots at all.
    Empty,
    /// Two `Required` spines target the same scope kind.
    ///
    /// Merging two contracts yields a third nobody wrote, so this is refused rather than reconciled.
    /// Soften one to `Preferred`, or move it to a different scope level — spines at different levels
    /// **compose**, which is also how a dungeon sits inside a Reach.
    ConflictingRequiredSpines {
        /// The scope kind both claim.
        kind: NodeKind,
        /// The first template.
        first: ObjectId,
        /// The second.
        second: ObjectId,
    },
}

impl fmt::Display for SpineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpineError::BudgetTooSmall { needed, available } => write!(
                f,
                "spine needs {needed} scopes (required slots + segment minimums); {available} are available"
            ),
            SpineError::ContentUnavailable { slot, content } => {
                write!(f, "slot {slot:?} requires {content}, which is not registered")
            }
            SpineError::UnknownSlot { referenced_by, name } => {
                write!(f, "{referenced_by:?} references slot {name:?}, which does not exist")
            }
            SpineError::DuplicateSlot { name } => {
                write!(f, "two slots are named {name:?}; references would be ambiguous")
            }
            SpineError::GrantOrderViolation { slot, granted_by } => write!(
                f,
                "slot {slot:?} requires what {granted_by:?} grants, but {granted_by:?} comes later — \
                 no route could satisfy it"
            ),
            SpineError::DegreeInfeasible { slot, needed, available } => write!(
                f,
                "slot {slot:?} needs {needed} connections; only {available} scopes are available to \
                 connect to"
            ),
            SpineError::DegreeContradiction {
                slot,
                max,
                required,
                because,
            } => write!(
                f,
                "slot {slot:?} caps connections at {max} but needs {required} ({because}); \
                 raise max_degree or drop a requirement"
            ),
            SpineError::EmptyContradiction { slot, because } => write!(
                f,
                "slot {slot:?} is declared empty but also declares {because}; the generator cannot \
                 both place that and place nothing"
            ),
            SpineError::Empty => write!(f, "a spine with no slots constrains nothing"),
            SpineError::ConflictingRequiredSpines {
                kind,
                first,
                second,
            } => write!(
                f,
                "spines {first} and {second} are both Required on {}; two contracts for one scope \
                 cannot both be honoured — soften one, or move it to a different scope level",
                kind.as_str()
            ),
        }
    }
}

impl std::error::Error for SpineError {}

/// A soft condition unlikely to hold — surfaced while authoring, never blocking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpineWarning {
    /// The slot or segment concerned.
    pub subject: String,
    /// What is unlikely, and why.
    pub detail: String,
}

impl fmt::Display for SpineWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.subject, self.detail)
    }
}

/// The result of validating a spine against a registry and a scope budget.
///
/// **Validation is the feature, not a safety net.** A guarantee is only useful if failure is discovered
/// while authoring, with the arithmetic shown — not during a player's session, and never as a silently
/// missing room.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpineValidation {
    /// `Required` conditions that cannot hold. Non-empty means generation must not proceed.
    pub errors: Vec<SpineError>,
    /// Soft conditions unlikely to hold. Informational.
    pub warnings: Vec<SpineWarning>,
}

impl SpineValidation {
    /// May generation proceed?
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

impl SpineTemplate {
    /// Check this spine against the registry and the scopes an instance will have.
    ///
    /// Only `Required` conditions can *fail*; softer tiers produce warnings, so a dev learns "your
    /// landmark will rarely fit" without being blocked by it.
    pub fn validate(&self, registry: &ContentRegistry, available_scopes: u32) -> SpineValidation {
        let mut v = SpineValidation::default();

        if self.slots.is_empty() {
            v.errors.push(SpineError::Empty);
            return v;
        }

        // Names must be unique, or every reference below is ambiguous.
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for slot in &self.slots {
            if !seen.insert(slot.name.as_str()) {
                v.errors.push(SpineError::DuplicateSlot {
                    name: slot.name.clone(),
                });
            }
        }

        // Budget: the spine raises a floor rather than competing with L1's count.
        let needed = self.required_minimum();
        if needed > available_scopes {
            v.errors.push(SpineError::BudgetTooSmall {
                needed,
                available: available_scopes,
            });
        }
        // Soft slots that will not fit are a warning, not a failure — that is what the tier means.
        let soft_total = self.slots.len() as u32;
        if soft_total > available_scopes && needed <= available_scopes {
            v.warnings.push(SpineWarning {
                subject: self.id.to_string(),
                detail: format!(
                    "{soft_total} slots declared but only {available_scopes} scopes available; \
                     softer slots will be dropped and reported"
                ),
            });
        }

        let index_of = |name: &str| self.slots.iter().position(|s| s.name == name);

        for (i, slot) in self.slots.iter().enumerate() {
            let required = slot.effective_strictness(self.strictness).is_required();

            // Content must exist to be demanded.
            for content in &slot.contents.must_contain {
                if !registry.contains(*content) {
                    if required {
                        v.errors.push(SpineError::ContentUnavailable {
                            slot: slot.name.clone(),
                            content: *content,
                        });
                    } else {
                        v.warnings.push(SpineWarning {
                            subject: slot.name.clone(),
                            detail: format!(
                                "{content} is not registered; this slot will be dropped"
                            ),
                        });
                    }
                }
            }
            for content in &slot.contents.prefer_contain {
                if !registry.contains(*content) {
                    v.warnings.push(SpineWarning {
                        subject: slot.name.clone(),
                        detail: format!("preferred content {content} is not registered"),
                    });
                }
            }

            // An empty slot cannot also be told what to hold. Choosing a winner silently would drop
            // one of the dev's own statements; saying so lets them decide which they meant.
            if slot.contents.empty {
                let demands = [
                    (!slot.contents.must_contain.is_empty()).then_some("must_contain"),
                    (!slot.contents.prefer_contain.is_empty()).then_some("prefer_contain"),
                    slot.contents.grants.is_some().then_some("grants"),
                ];
                for what in demands.into_iter().flatten() {
                    v.errors.push(SpineError::EmptyContradiction {
                        slot: slot.name.clone(),
                        because: what.into(),
                    });
                }
            }

            // Adjacency and grant references must name real slots...
            for target in &slot.shape.adjacent_to {
                if index_of(target).is_none() {
                    v.errors.push(SpineError::UnknownSlot {
                        referenced_by: slot.name.clone(),
                        name: target.clone(),
                    });
                }
            }
            // ...and a requirement must be granted by something *earlier*, or no route satisfies it.
            if let Some(UnlockRef::GrantedBy(source)) = &slot.contents.requires {
                match index_of(source) {
                    None => v.errors.push(SpineError::UnknownSlot {
                        referenced_by: slot.name.clone(),
                        name: source.clone(),
                    }),
                    Some(j) if j >= i => v.errors.push(SpineError::GrantOrderViolation {
                        slot: slot.name.clone(),
                        granted_by: source.clone(),
                    }),
                    Some(_) => {}
                }
            }

            // A ceiling has to fit everything the same slot also asked for. Counting, in order: the
            // route in (every slot but the first has one), each declared adjacency, and any floor.
            if let Some(max) = slot.shape.max_degree {
                let inbound = u32::from(i > 0);
                // Adjacencies already implied by the spine's own chain are not additional edges.
                let extra_adjacent = slot
                    .shape
                    .adjacent_to
                    .iter()
                    .filter(|t| index_of(t).is_some_and(|j| j + 1 != i && i + 1 != j))
                    .count() as u32;
                let needed = inbound + extra_adjacent;
                if needed > max {
                    v.errors.push(SpineError::DegreeContradiction {
                        slot: slot.name.clone(),
                        max,
                        required: needed,
                        because: format!(
                            "{inbound} route in + {extra_adjacent} declared adjacenc{}",
                            if extra_adjacent == 1 { "y" } else { "ies" }
                        ),
                    });
                }
                if let Some(min) = slot.shape.min_degree {
                    if min > max {
                        v.errors.push(SpineError::DegreeContradiction {
                            slot: slot.name.clone(),
                            max,
                            required: min,
                            because: "min_degree".into(),
                        });
                    }
                }
            }

            // A degree requirement implies that many neighbours must exist.
            if let Some(degree) = slot.shape.min_degree {
                if required && degree >= available_scopes {
                    v.errors.push(SpineError::DegreeInfeasible {
                        slot: slot.name.clone(),
                        needed: degree,
                        available: available_scopes.saturating_sub(1),
                    });
                }
            }
        }

        // Segments must connect slots that exist, in order.
        for segment in &self.segments {
            for name in [&segment.from, &segment.to] {
                if index_of(name).is_none() {
                    v.errors.push(SpineError::UnknownSlot {
                        referenced_by: format!("segment {}→{}", segment.from, segment.to),
                        name: name.clone(),
                    });
                }
            }
            if let Some(UnlockRef::GrantedBy(source)) = &segment.gated_by {
                match (index_of(source), index_of(&segment.to)) {
                    (None, _) => v.errors.push(SpineError::UnknownSlot {
                        referenced_by: format!("segment {}→{}", segment.from, segment.to),
                        name: source.clone(),
                    }),
                    (Some(g), Some(t)) if g > t => v.errors.push(SpineError::GrantOrderViolation {
                        slot: format!("segment {}→{}", segment.from, segment.to),
                        granted_by: source.clone(),
                    }),
                    _ => {}
                }
            }
        }

        v
    }
}

// ---------------------------------------------------------------------------------------------
// Instantiation
// ---------------------------------------------------------------------------------------------

/// Which scope a slot was allocated to, and what it granted.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotAssignment {
    /// The slot's declared name.
    pub slot: String,
    /// The scope it was allocated.
    pub scope: Handle<Node>,
    /// The unlock resolved from its [`GrantSpec`], if it had one.
    pub granted: Option<ObjectId>,
}

/// A soft slot that was dropped, and why.
///
/// **Graceful degradation that is invisible is just breakage**, so every relaxation is reported.
#[derive(Clone, Debug, PartialEq)]
pub struct Relaxation {
    /// The slot that was dropped.
    pub slot: String,
    /// Why.
    pub reason: String,
}

impl fmt::Display for Relaxation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "spine slot {:?} skipped: {}", self.slot, self.reason)
    }
}

/// One instantiated spine over one scope.
#[derive(Clone, Debug, PartialEq)]
pub struct SpineInstance {
    /// The template.
    pub template: ObjectId,
    /// The scope this instance covers.
    pub scope: Handle<Node>,
    /// Slots that were placed.
    pub assignments: Vec<SlotAssignment>,
    /// Soft slots that were dropped.
    pub relaxations: Vec<Relaxation>,
    /// Scopes whose slot declared [`empty`](SpineSlot::empty) — the generator must place nothing here.
    pub empty: Vec<Handle<Node>>,
    /// The scope this instance's [`SlotRole::Start`] slot landed on, if it declared one.
    pub start: Option<Handle<Node>>,
    /// The scope this instance's [`SlotRole::Goal`] slot landed on, if it declared one.
    pub goal: Option<Handle<Node>>,
}

impl SpineInstance {
    /// The scope allocated to a named slot.
    pub fn scope_of(&self, slot: &str) -> Option<Handle<Node>> {
        self.assignments
            .iter()
            .find(|a| a.slot == slot)
            .map(|a| a.scope)
    }

    /// The scopes the generator must leave empty.
    ///
    /// The L2 half is applied automatically at instantiation; feed this to
    /// [`Scheduler::excluding`](crate::schedule::Scheduler::excluding) for the L1 half, which plans
    /// against the scope graph and cannot see the mission graph's decisions.
    pub fn empty_scopes(&self) -> &[Handle<Node>] {
        &self.empty
    }

    /// The unlock a named slot granted.
    pub fn granted_by(&self, slot: &str) -> Option<ObjectId> {
        self.assignments
            .iter()
            .find(|a| a.slot == slot)
            .and_then(|a| a.granted)
    }
}

/// Instantiates spines onto a mission graph.
///
/// The spine **seeds the skeleton**: slots are allocated in order, segments grow between them, and
/// symbolic grants resolve — then M09's assumed fill and M10's un-softlockable pass run unchanged, the
/// latter retaining final say. With no spine registered, none of this runs and the result is exactly
/// M09.
pub struct SpineInstantiator<'a> {
    graph: &'a NodeGraph,
    templates: Vec<SpineTemplate>,
}

impl<'a> SpineInstantiator<'a> {
    /// An instantiator over a scope graph.
    pub fn new(graph: &'a NodeGraph) -> Self {
        SpineInstantiator {
            graph,
            templates: Vec::new(),
        }
    }

    /// Register a template.
    pub fn with_template(mut self, template: SpineTemplate) -> Self {
        self.templates.push(template);
        self
    }

    /// How many templates are registered. Zero is the normal case.
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Is no spine registered? Then generation is free-form.
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// The registered templates.
    pub fn templates(&self) -> &[SpineTemplate] {
        &self.templates
    }

    /// Validate every template, plus the rules that only make sense across templates.
    ///
    /// Call this **before** [`instantiate`](Self::instantiate). Guarantees are only palatable if
    /// failure lands at authoring time, in an editor, with the arithmetic on screen.
    pub fn validate(&self, registry: &ContentRegistry) -> SpineValidation {
        let mut v = SpineValidation::default();

        for template in &self.templates {
            let capacity = self
                .graph
                .walk()
                .into_iter()
                .filter(|h| {
                    self.graph
                        .get(*h)
                        .is_some_and(|n| n.kind() == template.applies_to)
                })
                .map(|h| self.capacity_of(h))
                .min()
                .unwrap_or(0);
            let one = template.validate(registry, capacity);
            v.errors.extend(one.errors);
            v.warnings.extend(one.warnings);
        }

        // Two hard contracts over one scope cannot both hold; refusing beats silently picking one.
        let mut required: BTreeMap<NodeKind, ObjectId> = BTreeMap::new();
        for template in &self.templates {
            if !template.strictness.is_required() {
                continue;
            }
            match required.get(&template.applies_to) {
                Some(first) => v.errors.push(SpineError::ConflictingRequiredSpines {
                    kind: template.applies_to,
                    first: *first,
                    second: template.id,
                }),
                None => {
                    required.insert(template.applies_to, template.id);
                }
            }
        }

        v
    }

    /// How many Spaces sit inside a scope — the budget a spine over it has to fit in.
    fn capacity_of(&self, scope: Handle<Node>) -> u32 {
        self.graph
            .descendants_of(scope)
            .into_iter()
            .filter(|h| {
                self.graph
                    .get(*h)
                    .is_some_and(|n| n.kind() == NodeKind::Space)
            })
            .count() as u32
    }

    /// Instantiate every registered template onto the mission graph.
    ///
    /// Returns one [`SpineInstance`] per covered scope instance. Deterministic: instances are visited
    /// in graph-walk order and each draws from a stream forked on the scope's identity.
    pub fn instantiate(&self, mission: &mut MissionGraph, rng: &Rng) -> Vec<SpineInstance> {
        let mut out = Vec::new();
        for template in &self.templates {
            let instances: Vec<Handle<Node>> = self
                .graph
                .walk()
                .into_iter()
                .filter(|h| {
                    self.graph
                        .get(*h)
                        .map(|n| n.kind() == template.applies_to)
                        .unwrap_or(false)
                })
                .collect();

            for index in template.coverage.selected(instances.len()) {
                let scope = instances[index];
                if let Some(instance) = self.instantiate_one(template, scope, mission, rng) {
                    out.push(instance);
                }
            }
        }

        // Roles are wired *after* every instance exists, because a repeating spine declares its roles
        // once per instance and the world has exactly one of each.
        //
        // **First start, last goal.** A per-Reach spine saying "each Reach begins here and ends there"
        // means the world begins at the *first* such room and is completed at the *last* — which is
        // both the intuitive reading and the only one that stays coherent as Reaches are added.
        // Last-writer-wins would have put the start in whichever Reach happened to be visited last.
        if let Some(start) = out.iter().find_map(|i| i.start) {
            mission.set_start(start);
        }
        if let Some(goal) = out.iter().rev().find_map(|i| i.goal) {
            mission.set_goal(goal);
        }
        out
    }

    /// Instantiate one template over one scope instance.
    fn instantiate_one(
        &self,
        template: &SpineTemplate,
        scope: Handle<Node>,
        mission: &mut MissionGraph,
        rng: &Rng,
    ) -> Option<SpineInstance> {
        // Scopes available inside this instance, in deterministic walk order.
        let available: Vec<Handle<Node>> = self
            .graph
            .descendants_of(scope)
            .into_iter()
            .filter(|h| {
                self.graph
                    .get(*h)
                    .map(|n| n.kind() == NodeKind::Space)
                    .unwrap_or(false)
            })
            .collect();

        let node_id = self.graph.get(scope)?.id().to_string();
        let local = rng
            .fork("spine")
            .fork(&template.id.to_string())
            .fork(&node_id);

        let mut assignments: Vec<SlotAssignment> = Vec::new();
        let mut relaxations: Vec<Relaxation> = Vec::new();
        let mut cursor = 0usize;

        // Allocate slots in order, growing segments between them. Lengths are planned up front so
        // surplus reaches the *later* segments too — consuming greedily front-to-back would leave the
        // capstone jammed against the start in any generously sized scope.
        let kept: Vec<&SpineSlot> = template.kept_slots();
        let lengths = template.segment_lengths(&kept, available.len());
        let mut gap = 0usize;
        for slot in &template.slots {
            if !kept.iter().any(|k| k.name == slot.name) {
                relaxations.push(Relaxation {
                    slot: slot.name.clone(),
                    reason: format!(
                        "{} at adherence {:.2}",
                        slot.effective_strictness(template.strictness),
                        slot.adherence.unwrap_or(template.adherence)
                    ),
                });
                continue;
            }

            // The segment preceding this slot consumes connective tissue first.
            if let Some(prev) = assignments.last() {
                let length = lengths.get(gap).copied().unwrap_or(0) as usize;
                gap += 1;
                // Never consume the last scope on filler — the slot itself has to land somewhere.
                let end = (cursor + length).min(available.len().saturating_sub(1));
                let end = end.max(cursor);
                let filler: Vec<Handle<Node>> = available[cursor..end].to_vec();
                cursor = end;

                // Chain: prev → filler… → (the slot allocated below).
                let mut chain_from = prev.scope;
                for step in &filler {
                    if !mission.connects(chain_from, *step) {
                        mission.add_edge(MissionEdge::open(chain_from, *step));
                    }
                    chain_from = *step;
                }
                // Stash where the chain ended so the slot connects to it.
                if cursor < available.len() {
                    let slot_scope = available[cursor];
                    cursor += 1;
                    if !mission.connects(chain_from, slot_scope) {
                        mission.add_edge(MissionEdge::open(chain_from, slot_scope));
                    }
                    assignments.push(self.assign(slot, slot_scope, &local));
                    continue;
                }
                relaxations.push(Relaxation {
                    slot: slot.name.clone(),
                    reason: "no scope left in this instance".into(),
                });
                continue;
            }

            // The first slot simply takes the first available scope.
            if cursor < available.len() {
                let slot_scope = available[cursor];
                cursor += 1;
                assignments.push(self.assign(slot, slot_scope, &local));
            } else {
                relaxations.push(Relaxation {
                    slot: slot.name.clone(),
                    reason: "no scope left in this instance".into(),
                });
            }
        }

        // Explicit adjacency between slots, and gating resolved from symbolic references.
        self.wire_adjacency(template, &assignments, mission);
        self.apply_gating(template, &assignments, mission);
        self.satisfy_degrees(template, &assignments, &available, mission);
        // Caps go on last: everything above is structure the spine *promised*, and a promise must not
        // be blocked by the ceiling that protects it.
        self.apply_degree_caps(template, &assignments, mission, &mut relaxations);

        let placed = |name: &str| assignments.iter().find(|a| a.slot == name).map(|a| a.scope);

        // Emptiness: the L2 half. L1 is told via `SpineInstance::empty_scopes`, since scheduling runs
        // against the scope graph and never sees this.
        let mut empty = Vec::new();
        for slot in template.slots.iter().filter(|s| s.contents.empty) {
            if let Some(scope) = placed(&slot.name) {
                mission.exclude_content(scope);
                empty.push(scope);
            }
        }

        let start = template
            .slots
            .iter()
            .find(|s| s.role == SlotRole::Start)
            .and_then(|s| placed(&s.name));
        let goal = template
            .slots
            .iter()
            .find(|s| s.role == SlotRole::Goal)
            .and_then(|s| placed(&s.name));

        Some(SpineInstance {
            template: template.id,
            scope,
            assignments,
            relaxations,
            empty,
            start,
            goal,
        })
    }

    /// Allocate a slot, resolving its grant.
    fn assign(&self, slot: &SpineSlot, scope: Handle<Node>, rng: &Rng) -> SlotAssignment {
        let granted = slot.contents.grants.as_ref().and_then(|spec| {
            if spec.any_of.is_empty() {
                return None;
            }
            // Deterministic choice from the declared candidates — the generator picks, the dev
            // constrains. Forked on the slot name so one slot's choice does not shift another's.
            let mut picker = rng.fork("grant").fork(&slot.name);
            let index = picker.below(spec.any_of.len() as u64) as usize;
            Some(spec.any_of[index])
        });
        SlotAssignment {
            slot: slot.name.clone(),
            scope,
            granted,
        }
    }

    /// Add the direct connections `adjacent_to` demands.
    fn wire_adjacency(
        &self,
        template: &SpineTemplate,
        assignments: &[SlotAssignment],
        mission: &mut MissionGraph,
    ) {
        let placed: BTreeMap<&str, Handle<Node>> = assignments
            .iter()
            .map(|a| (a.slot.as_str(), a.scope))
            .collect();
        for slot in &template.slots {
            let Some(from) = placed.get(slot.name.as_str()) else {
                continue;
            };
            for target in &slot.shape.adjacent_to {
                if let Some(to) = placed.get(target.as_str()) {
                    if !mission.connects(*from, *to) {
                        mission.add_edge(MissionEdge::open(*from, *to));
                    }
                }
            }
        }
    }

    /// Resolve symbolic unlock references and gate the corresponding edges.
    fn apply_gating(
        &self,
        template: &SpineTemplate,
        assignments: &[SlotAssignment],
        mission: &mut MissionGraph,
    ) {
        let resolve = |r: &UnlockRef| -> Option<ObjectId> {
            match r {
                UnlockRef::Explicit(c) => Some(*c),
                UnlockRef::GrantedBy(slot) => assignments
                    .iter()
                    .find(|a| a.slot == *slot)
                    .and_then(|a| a.granted),
            }
        };
        let placed: BTreeMap<&str, Handle<Node>> = assignments
            .iter()
            .map(|a| (a.slot.as_str(), a.scope))
            .collect();

        for segment in &template.segments {
            let Some(gate) = segment.gated_by.as_ref().and_then(resolve) else {
                continue;
            };
            let (Some(from), Some(to)) = (
                placed.get(segment.from.as_str()),
                placed.get(segment.to.as_str()),
            ) else {
                continue;
            };
            // Gate the first edge leaving the segment's origin toward its destination.
            let index = mission
                .edges()
                .iter()
                .position(|e| (e.from == *from || e.to == *from) && !e.is_gated());
            if let Some(i) = index {
                mission.gate_edge(i, Rule::has(gate));
            }
            let _ = to;
        }
    }

    /// Add connections until each slot meets its `min_degree`.
    ///
    /// **Requirements first, dials on the remainder:** these connections are added before
    /// `cycle_density` is applied, so a dev at zero cycles still gets their four-entrance arena.
    fn satisfy_degrees(
        &self,
        template: &SpineTemplate,
        assignments: &[SlotAssignment],
        available: &[Handle<Node>],
        mission: &mut MissionGraph,
    ) {
        for slot in &template.slots {
            let Some(degree) = slot.shape.min_degree else {
                continue;
            };
            let Some(assignment) = assignments.iter().find(|a| a.slot == slot.name) else {
                continue;
            };
            let scope = assignment.scope;
            let mut current = mission.degree(scope);

            for candidate in available {
                if current >= degree {
                    break;
                }
                if *candidate == scope || mission.connects(scope, *candidate) {
                    continue;
                }
                if mission
                    .add_edge(MissionEdge::open(scope, *candidate))
                    .is_some()
                {
                    current += 1;
                }
            }
        }
    }

    /// Freeze each capped slot's connectivity on the mission graph.
    ///
    /// Runs **last**, after the spine has wired its own guaranteed edges, so a cap never blocks the
    /// structure the spine itself promised. From here on the cap is the graph's business: every later
    /// pass is refused by [`MissionGraph::add_edge`] rather than asked to cooperate.
    ///
    /// A slot already over its cap when the spine arrives is **reported, not silently accepted** —
    /// caps look forward, and edges already in the graph cannot be torn out without invalidating every
    /// index the solver and the softlock pass hold.
    fn apply_degree_caps(
        &self,
        template: &SpineTemplate,
        assignments: &[SlotAssignment],
        mission: &mut MissionGraph,
        relaxations: &mut Vec<Relaxation>,
    ) {
        for slot in &template.slots {
            let Some(cap) = slot.shape.max_degree else {
                continue;
            };
            let Some(assignment) = assignments.iter().find(|a| a.slot == slot.name) else {
                continue;
            };
            let scope = assignment.scope;
            let actual = mission.degree(scope);
            if actual > cap {
                relaxations.push(Relaxation {
                    slot: slot.name.clone(),
                    reason: format!(
                        "max_degree {cap} could not be applied: the scope already had {actual} \
                         connections when the spine ran — seed the spine into an empty MissionGraph \
                         and call connect_scopes afterwards"
                    ),
                });
            }
            mission.set_degree_cap(scope, cap);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------------------------

use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};

impl Serialize for Strictness {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.tag());
    }
}

impl Deserialize for Strictness {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => Strictness::Required,
            1 => Strictness::Preferred,
            2 => Strictness::Optional,
            _ => return Err(SerError::InvalidValue("unknown Strictness tag")),
        })
    }
}

impl Serialize for UnlockRef {
    fn serialize(&self, w: &mut Writer) {
        match self {
            UnlockRef::Explicit(c) => {
                w.u8(0);
                w.write(c);
            }
            UnlockRef::GrantedBy(slot) => {
                w.u8(1);
                w.str(slot);
            }
        }
    }
}

impl Deserialize for UnlockRef {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(match r.u8()? {
            0 => UnlockRef::Explicit(r.read()?),
            1 => UnlockRef::GrantedBy(r.str()?),
            _ => return Err(SerError::InvalidValue("unknown UnlockRef tag")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentKind;
    use crate::node::NodeState;
    use cv_determinism::{Aabb, Vec3};

    fn oid(ns: &str, name: &str) -> ObjectId {
        ObjectId::derived(ns, name)
    }

    /// A world of `reaches` Reaches, each holding `per` Spaces.
    fn world(reaches: usize, per: usize) -> (NodeGraph, Vec<Handle<Node>>) {
        let mut g = NodeGraph::new(1.0, 1);
        let mut reach_handles = Vec::new();
        for r in 0..reaches {
            let reach = g.add_child(g.root(), format!("reach_{r}")).unwrap();
            let area = g.add_child(reach, format!("area_{r}")).unwrap();
            for s in 0..per {
                g.add_child(area, format!("space_{r}_{s}")).unwrap();
            }
            reach_handles.push(reach);
        }
        for h in g.walk() {
            g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
                .unwrap();
        }
        for h in g.walk() {
            g.advance(h, NodeState::Realized).unwrap();
        }
        (g, reach_handles)
    }

    /// Example's loop: start → <anything> → capstone(boss) → terminal(treasury, an exit of capstone).
    fn example_spine() -> SpineTemplate {
        SpineTemplate::new(oid("spine", "reach_loop"), NodeKind::Reach)
            .slot(SpineSlot::new("start").role(SlotRole::Start))
            .slot(SpineSlot::new("capstone").must_contain([oid("actor", "boss")]))
            .slot(
                SpineSlot::new("terminal")
                    .role(SlotRole::Goal)
                    .must_contain([oid("actor", "beacon")])
                    .adjacent_to("capstone"),
            )
            .segment(SpineSegment::new(
                "start",
                "capstone",
                AdaptiveRange::new(1, 4),
            ))
            .segment(SpineSegment::direct("capstone", "terminal"))
    }

    fn registry_with(paths: &[(&str, &str)]) -> ContentRegistry {
        let mut r = ContentRegistry::new();
        for (ns, path) in paths {
            let kind = match *ns {
                "actor" => ContentKind::Actor,
                "unlock" => ContentKind::Item,
                _ => ContentKind::Actor,
            };
            r.register(kind, *path, 1).unwrap();
        }
        r
    }

    #[test]
    fn adherence_never_weakens_a_required_slot() {
        // The safety property of the dial: guarantees live in a tier it cannot reach.
        let spine = example_spine().adherence(0.0);
        assert_eq!(
            spine.kept_slots().len(),
            3,
            "every slot is Required by default"
        );
        for slot in &spine.slots {
            assert!(spine.keeps(slot), "{} must survive adherence 0", slot.name);
        }
    }

    #[test]
    fn adherence_does_modulate_the_soft_tiers() {
        let build = |adherence: f64| {
            SpineTemplate::new(oid("spine", "loose"), NodeKind::Reach)
                .adherence(adherence)
                .slot(SpineSlot::new("start"))
                .slot(SpineSlot::new("landmark").strictness(Strictness::Preferred))
                .slot(SpineSlot::new("finale").strictness(Strictness::Optional))
        };
        assert_eq!(
            build(1.0).kept_slots().len(),
            3,
            "all three at full adherence"
        );
        assert_eq!(build(0.5).kept_slots().len(), 2, "the Optional one drops");
        assert_eq!(
            build(0.0).kept_slots().len(),
            1,
            "only the Required one survives"
        );
    }

    #[test]
    fn a_per_slot_override_beats_the_template() {
        // "Follow this loosely, but the capstone is non-negotiable" should be one line.
        let spine = SpineTemplate::new(oid("spine", "mixed"), NodeKind::Reach)
            .strictness(Strictness::Optional)
            .adherence(0.0)
            .slot(SpineSlot::new("filler"))
            .slot(SpineSlot::new("capstone").strictness(Strictness::Required));
        let kept = spine.kept_slots();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].name, "capstone");
    }

    #[test]
    fn the_budget_floor_is_the_slots_plus_segment_minimums() {
        let spine = example_spine();
        // 3 slots + segment minimums (1 for start→capstone, 0 for the direct exit).
        assert_eq!(spine.required_minimum(), 4);
    }

    #[test]
    fn validation_reports_a_budget_shortfall_with_the_arithmetic() {
        let registry = registry_with(&[("actor", "boss"), ("actor", "beacon")]);
        let spine = example_spine();
        let v = spine.validate(&registry, 2);
        assert!(!v.is_ok());
        let message = v.errors[0].to_string();
        assert!(message.contains("needs 4 scopes"), "{message}");
        assert!(message.contains("2 are available"), "{message}");
        // And it passes once the budget is adequate.
        assert!(spine.validate(&registry, 4).is_ok());
    }

    #[test]
    fn validation_catches_missing_content_and_bad_references() {
        // Nothing registered: the boss and beacon demands cannot be met.
        let v = example_spine().validate(&ContentRegistry::new(), 10);
        assert!(v
            .errors
            .iter()
            .any(|e| matches!(e, SpineError::ContentUnavailable { .. })));

        let dangling = SpineTemplate::new(oid("spine", "x"), NodeKind::Reach)
            .slot(SpineSlot::new("a").adjacent_to("nowhere"));
        let v = dangling.validate(&ContentRegistry::new(), 10);
        assert!(v
            .errors
            .iter()
            .any(|e| matches!(e, SpineError::UnknownSlot { .. })));

        let duplicate = SpineTemplate::new(oid("spine", "y"), NodeKind::Reach)
            .slot(SpineSlot::new("a"))
            .slot(SpineSlot::new("a"));
        assert!(duplicate
            .validate(&ContentRegistry::new(), 10)
            .errors
            .iter()
            .any(|e| matches!(e, SpineError::DuplicateSlot { .. })));

        assert!(SpineTemplate::new(oid("spine", "z"), NodeKind::Reach)
            .validate(&ContentRegistry::new(), 10)
            .errors
            .contains(&SpineError::Empty));
    }

    #[test]
    fn a_requirement_granted_only_later_is_rejected() {
        // The reference cycle the design names: a slot depending on something further along.
        let backwards = SpineTemplate::new(oid("spine", "cycle"), NodeKind::Reach)
            .slot(SpineSlot::new("start").requires(UnlockRef::GrantedBy("capstone".into())))
            .slot(SpineSlot::new("capstone").grants(GrantSpec::any_of([oid("unlock", "dash")])));
        let v = backwards.validate(&ContentRegistry::new(), 10);
        assert!(v
            .errors
            .iter()
            .any(|e| matches!(e, SpineError::GrantOrderViolation { .. })));
        // Forwards is fine.
        let forwards = SpineTemplate::new(oid("spine", "ok"), NodeKind::Reach)
            .slot(SpineSlot::new("precursor").grants(GrantSpec::any_of([oid("unlock", "dash")])))
            .slot(SpineSlot::new("capstone").requires(UnlockRef::GrantedBy("precursor".into())));
        assert!(forwards.validate(&ContentRegistry::new(), 10).is_ok());
    }

    #[test]
    fn soft_slots_warn_rather_than_fail() {
        // Only `Required` can fail — that is what the tier means.
        let spine = SpineTemplate::new(oid("spine", "soft"), NodeKind::Reach)
            .slot(SpineSlot::new("start"))
            .slot(
                SpineSlot::new("landmark")
                    .strictness(Strictness::Preferred)
                    .must_contain([oid("actor", "absent")]),
            );
        let v = spine.validate(&ContentRegistry::new(), 10);
        assert!(v.is_ok(), "a soft slot must not block generation");
        assert!(
            !v.warnings.is_empty(),
            "but it must be surfaced while authoring"
        );
        assert!(v.warnings[0].to_string().contains("landmark"));
    }

    #[test]
    fn surplus_spreads_across_segments_by_declared_headroom() {
        let spine = SpineTemplate::new(oid("spine", "spread"), NodeKind::Reach)
            .slot(SpineSlot::new("a"))
            .slot(SpineSlot::new("b"))
            .slot(SpineSlot::new("c"))
            // Headroom 3 and 1: the first segment asked to be able to run longer, so it gets more.
            .segment(SpineSegment::new("a", "b", AdaptiveRange::new(1, 4)))
            .segment(SpineSegment::new("b", "c", AdaptiveRange::new(1, 2)));
        let kept = spine.kept_slots();

        // Exactly the floor: 3 slots + 2 minimums.
        assert_eq!(spine.segment_lengths(&kept, 5), vec![1, 1]);
        // Four spare, split 3:1 by headroom, and neither segment exceeds its hard_max.
        assert_eq!(spine.segment_lengths(&kept, 9), vec![4, 2]);
        // More capacity than the declared ceilings can absorb — the extra is simply not used.
        assert_eq!(spine.segment_lengths(&kept, 40), vec![4, 2]);
        // Below the floor, nothing is inflated.
        assert_eq!(spine.segment_lengths(&kept, 2), vec![1, 1]);
    }

    #[test]
    fn surplus_reaches_the_later_slots_too() {
        // Consuming greedily front-to-back would jam the capstone against the start in a large Reach.
        let (g, _) = world(1, 12);
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(example_spine())
            .instantiate(&mut mission, &Rng::new(1));

        let instance = &instances[0];
        let spaces: Vec<_> = g.of_kind(NodeKind::Space).map(|(h, _)| h).collect();
        let position = |slot: &str| {
            spaces
                .iter()
                .position(|h| Some(*h) == instance.scope_of(slot))
                .unwrap()
        };
        // start→capstone declares 1..=4, so the capstone lands past the first few rooms.
        assert!(
            position("capstone") >= 4,
            "the capstone sat at {} in a 12-Space Reach",
            position("capstone")
        );
    }

    #[test]
    fn two_required_spines_over_one_scope_kind_is_refused() {
        // Merging two contracts yields a third nobody wrote.
        let (g, _) = world(1, 8);
        let clash = SpineInstantiator::new(&g)
            .with_template(
                SpineTemplate::new(oid("spine", "a"), NodeKind::Reach).slot(SpineSlot::new("x")),
            )
            .with_template(
                SpineTemplate::new(oid("spine", "b"), NodeKind::Reach).slot(SpineSlot::new("y")),
            );
        let v = clash.validate(&ContentRegistry::new());
        assert!(v
            .errors
            .iter()
            .any(|e| matches!(e, SpineError::ConflictingRequiredSpines { .. })));

        // Different scope levels compose — that is how a dungeon sits inside a Reach.
        let composed = SpineInstantiator::new(&g)
            .with_template(
                SpineTemplate::new(oid("spine", "a"), NodeKind::Reach).slot(SpineSlot::new("x")),
            )
            .with_template(
                SpineTemplate::new(oid("spine", "b"), NodeKind::Area).slot(SpineSlot::new("y")),
            );
        assert!(composed.validate(&ContentRegistry::new()).is_ok());

        // And softening one resolves the clash without moving it.
        let softened = SpineInstantiator::new(&g)
            .with_template(
                SpineTemplate::new(oid("spine", "a"), NodeKind::Reach).slot(SpineSlot::new("x")),
            )
            .with_template(
                SpineTemplate::new(oid("spine", "b"), NodeKind::Reach)
                    .strictness(Strictness::Preferred)
                    .slot(SpineSlot::new("y")),
            );
        assert!(softened.validate(&ContentRegistry::new()).is_ok());
    }

    #[test]
    fn instantiator_validation_uses_the_tightest_instance_as_the_budget() {
        // A spine that fits three Reaches and not the fourth is a spine that does not fit.
        let mut g = NodeGraph::new(1.0, 1);
        for (r, spaces) in [8usize, 8, 2].into_iter().enumerate() {
            let reach = g.add_child(g.root(), format!("reach_{r}")).unwrap();
            let area = g.add_child(reach, format!("area_{r}")).unwrap();
            for s in 0..spaces {
                g.add_child(area, format!("space_{r}_{s}")).unwrap();
            }
        }
        let registry = registry_with(&[("actor", "boss"), ("actor", "beacon")]);
        let v = SpineInstantiator::new(&g)
            .with_template(example_spine())
            .validate(&registry);
        assert!(v
            .errors
            .iter()
            .any(|e| matches!(e, SpineError::BudgetTooSmall { available: 2, .. })));
    }

    #[test]
    fn coverage_is_a_pattern_not_a_lottery() {
        assert_eq!(Coverage::All.selected(4), vec![0, 1, 2, 3]);
        assert_eq!(
            Coverage::Every(3).selected(7),
            vec![0, 3, 6],
            "every third, exactly"
        );
        assert_eq!(Coverage::Indices(vec![1, 3, 9]).selected(4), vec![1, 3]);
        // A fraction spreads evenly by accumulation rather than sampling per instance.
        let half = Coverage::Fraction(Curve::constant(0.5)).selected(6);
        assert_eq!(half.len(), 3, "half of six, deterministically");
        assert_eq!(half, vec![1, 3, 5]);
        // And it is reproducible.
        assert_eq!(Coverage::Fraction(Curve::constant(0.5)).selected(6), half);
    }

    #[test]
    fn instantiation_places_every_required_slot() {
        let (g, reaches) = world(2, 6);
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(example_spine())
            .instantiate(&mut mission, &Rng::new(1));

        assert_eq!(instances.len(), 2, "the spine repeats per Reach");
        for instance in &instances {
            assert!(
                instance.relaxations.is_empty(),
                "{:?}",
                instance.relaxations
            );
            for name in ["start", "capstone", "terminal"] {
                assert!(instance.scope_of(name).is_some(), "{name} was not placed");
            }
            // Each instance draws from its own Reach.
            assert_eq!(
                g.scope_of(instance.scope_of("capstone").unwrap(), NodeKind::Reach),
                Some(instance.scope)
            );
            let _ = reaches;
        }
    }

    #[test]
    fn the_terminal_is_directly_connected_to_the_capstone() {
        // Example's requirement: the Rest Treasury is an exit of the boss chamber.
        let (g, _) = world(1, 6);
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(example_spine())
            .instantiate(&mut mission, &Rng::new(1));

        let instance = &instances[0];
        let capstone = instance.scope_of("capstone").unwrap();
        let terminal = instance.scope_of("terminal").unwrap();
        assert!(
            mission.connects(capstone, terminal),
            "the treasury must be an exit of the capstone"
        );
    }

    #[test]
    fn a_symbolic_grant_resolves_once_and_is_used_everywhere() {
        // The Zelda pattern: the precursor grants *something*, and the path onward is gated on it.
        let dash = oid("unlock", "dash");
        let grapple = oid("unlock", "grapple");
        let spine = SpineTemplate::new(oid("spine", "dungeon"), NodeKind::Area)
            .slot(SpineSlot::new("start").role(SlotRole::Start))
            .slot(SpineSlot::new("precursor").grants(GrantSpec::any_of([dash, grapple])))
            .slot(
                SpineSlot::new("capstone")
                    .role(SlotRole::Goal)
                    .requires(UnlockRef::GrantedBy("precursor".into())),
            )
            .segment(SpineSegment::new(
                "start",
                "precursor",
                AdaptiveRange::new(1, 2),
            ))
            .segment(
                SpineSegment::new("precursor", "capstone", AdaptiveRange::new(1, 2))
                    .gated_by(UnlockRef::GrantedBy("precursor".into())),
            );

        let (g, _) = world(1, 8);
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(spine)
            .instantiate(&mut mission, &Rng::new(3));

        let granted = instances[0]
            .granted_by("precursor")
            .expect("the precursor grants something");
        assert!(
            granted == dash || granted == grapple,
            "chosen from the declared candidates"
        );
        // The same unlock now gates an edge — the theme holds however it resolved.
        assert!(
            mission
                .edges()
                .iter()
                .any(|e| e.rule.unlocks().contains(&granted)),
            "the segment must be gated on what the precursor actually granted"
        );
    }

    #[test]
    fn relaxations_are_reported_not_silent() {
        let (g, _) = world(1, 6);
        let spine = SpineTemplate::new(oid("spine", "loose"), NodeKind::Reach)
            .adherence(0.0)
            .slot(SpineSlot::new("start"))
            .slot(SpineSlot::new("landmark").strictness(Strictness::Preferred))
            .slot(SpineSlot::new("finale").strictness(Strictness::Optional));

        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(spine)
            .instantiate(&mut mission, &Rng::new(1));

        let instance = &instances[0];
        assert_eq!(
            instance.assignments.len(),
            1,
            "only the Required slot survives"
        );
        assert_eq!(instance.relaxations.len(), 2, "and both drops are reported");
        let text = instance.relaxations[0].to_string();
        assert!(text.contains("skipped"), "{text}");
        assert!(
            text.contains("landmark") || text.contains("finale"),
            "{text}"
        );
    }

    #[test]
    fn min_degree_adds_connections() {
        let (g, _) = world(1, 8);
        let spine = SpineTemplate::new(oid("spine", "hub"), NodeKind::Reach)
            .slot(SpineSlot::new("hub").min_degree(4))
            .slot(SpineSlot::new("far"))
            .segment(SpineSegment::new("hub", "far", AdaptiveRange::new(1, 1)));

        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(spine)
            .instantiate(&mut mission, &Rng::new(1));

        let hub = instances[0].scope_of("hub").unwrap();
        let degree = mission
            .edges()
            .iter()
            .filter(|e| e.from == hub || e.to == hub)
            .count();
        assert!(
            degree >= 4,
            "the four-entrance requirement must be met, got {degree}"
        );
    }

    #[test]
    fn coverage_limits_which_instances_receive_the_spine() {
        let (g, _) = world(6, 5);
        let spine = example_spine().coverage(Coverage::Every(3));
        let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
        let instances = SpineInstantiator::new(&g)
            .with_template(spine)
            .instantiate(&mut mission, &Rng::new(1));
        assert_eq!(instances.len(), 2, "reaches 0 and 3 of six");
    }

    #[test]
    fn instantiation_is_deterministic() {
        let (g, _) = world(2, 6);
        let run = || {
            let mut mission = MissionGraph::new(g.of_kind(NodeKind::Space).next().unwrap().0);
            let instances = SpineInstantiator::new(&g)
                .with_template(example_spine())
                .instantiate(&mut mission, &Rng::new(0xABC));
            (mission, instances)
        };
        let (m1, i1) = run();
        let (m2, i2) = run();
        assert_eq!(m1, m2);
        assert_eq!(i1, i2);
    }

    #[test]
    fn no_registered_spine_does_nothing_at_all() {
        // The opt-in default: registering none must leave generation untouched.
        let (g, _) = world(2, 5);
        let start = g.of_kind(NodeKind::Space).next().unwrap().0;
        let mut mission = MissionGraph::from_scopes(&g, start);
        let before = mission.clone();

        let instantiator = SpineInstantiator::new(&g);
        assert!(instantiator.is_empty());
        let instances = instantiator.instantiate(&mut mission, &Rng::new(1));

        assert!(instances.is_empty());
        assert_eq!(mission, before, "no spine must mean no change whatsoever");
    }
}
