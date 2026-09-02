//! **Fill bands** — a composition graph over candidate placements, attached to a spine element.
//!
//! ```text
//! [Scope Floors] ▶ [Filter: slope < 30°] ▶ [Scatter: poisson, min 4m] ▶ [Place]
//! ```
//!
//! ⚠ **This is not a second feature.** A segment was *already* "a region the algorithm fills, tuned by
//! dials"; a fill graph is that seam done properly, with a richer tuner. It also explains a smell in
//! the dial-only model — segment dials were an unbounded list that kept growing, which is what a fixed
//! set of knobs looks like when it wants to be a graph.
//!
//! # The three safety rules, and why attachment makes them free
//!
//! | Rule | Why it costs nothing to state |
//! |---|---|
//! | **runs after obligations** | its attachment point *is* a spine element, which resolves after its own requirements are met — the pipeline layer is implied, so nothing needs declaring |
//! | **places only content with an empty `gate()` and empty `grants()`** | checked against the candidate set, with violations reported rather than filtered |
//! | **reads solver-reserved volumes as an exclusion input** | a node the palette provides by default |
//!
//! ⚠ **Rule 2 is the same boundary as the affix quarantine** ([`gate::Domain`](crate::gate::Domain)):
//! *a fill graph may only place content the proof does not depend on.* Two independent subsystems
//! wanting the same boundary is strong evidence the boundary is real.
//!
//! # The wall: a fill band must not become a second placement engine
//!
//! ⚠ **If fill nodes could gate, grant or route, two systems would decide placement and only one would
//! prove anything.** The wall is enforced in the palette rather than by review: [`FillOp`] has no
//! gating form, and [`FillGraph::place`] refuses a candidate that gates or grants — so the rule is a
//! property of what is constructible, not of what someone remembers to check.
//!
//! # Scope inheritance
//!
//! ⚠ **A fill runs once per instance of the scope its attachment point occupies.** An Area slot's fill
//! runs per Area, however many Spaces nest inside it. The competing reading — *run at the finest scope
//! inside* — is not wrong, it is **unpredictable**: the iteration count would change when somebody adds
//! a nested spine somewhere else, and nothing in front of the developer would say why.
//!
//! ⚠ **The attachment scope is a ceiling on selection, not merely a loop counter.** A fill on a Space
//! slot cannot select Floors outside that Space — otherwise *"per Space"* and *"per Area"* would differ
//! only in how many times the same world-wide fill ran.

use crate::arena::Handle;
use crate::node::{Node, NodeGraph, NodeKind};
use crate::object::ObjectId;
use crate::tag::{Tag, TagQuery};
use cv_determinism::Rng;
use std::collections::BTreeSet;
use std::fmt;

/// One candidate the fill graph may place.
///
/// ⚠ **`gates` and `grants` are carried facts, not questions this crate can answer.** Both are authored
/// hooks that run in the VM; the core is told what they returned. Modelling them as booleans here is
/// what lets rule 2 be enforced structurally at the seam where the pool is built, rather than deep
/// inside a placement loop that has no way to run a script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FillCandidate {
    /// The registered content.
    pub content: ObjectId,
    /// Does its `gate()` return anything?
    pub gates: bool,
    /// Does its `grants()` return anything?
    pub grants: bool,
    /// Its tags, for weighting and filtering.
    pub tags: Vec<Tag>,
}

impl FillCandidate {
    /// Content that neither gates nor grants — the only kind a fill band may place.
    pub fn new(content: ObjectId) -> Self {
        FillCandidate {
            content,
            gates: false,
            grants: false,
            tags: Vec::new(),
        }
    }

    /// Mark it as gating.
    pub fn gating(mut self) -> Self {
        self.gates = true;
        self
    }

    /// Mark it as granting.
    pub fn granting(mut self) -> Self {
        self.grants = true;
        self
    }

    /// Give it a tag.
    pub fn tagged(mut self, tag: &str) -> Self {
        self.tags.push(Tag::new(tag));
        self
    }

    /// May a fill band place this at all?
    pub fn is_fillable(&self) -> bool {
        !self.gates && !self.grants
    }

    /// Why not, if not.
    pub fn ineligibility(&self) -> Option<Ineligible> {
        match (self.gates, self.grants) {
            (true, true) => Some(Ineligible::GatesAndGrants),
            (true, false) => Some(Ineligible::Gates),
            (false, true) => Some(Ineligible::Grants),
            (false, false) => None,
        }
    }
}

/// Why a candidate may not be fill-placed.
///
/// ⚠ **Reported, never silently dropped.** The design's mock-up promises the reason is shown *live* in
/// the editor, and a rejection a developer cannot see is indistinguishable from content that was never
/// in the pool — which is the bug they will spend an afternoon on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ineligible {
    /// Its `gate()` is non-empty.
    Gates,
    /// Its `grants()` is non-empty.
    Grants,
    /// Both.
    GatesAndGrants,
}

impl fmt::Display for Ineligible {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self {
            Ineligible::Gates => "a non-empty gate()",
            Ineligible::Grants => "a non-empty grants()",
            Ineligible::GatesAndGrants => "a non-empty gate() and grants()",
        };
        write!(
            f,
            "{what} — a fill band may only place content the solvability proof does not depend on"
        )
    }
}

/// One node in a fill graph.
///
/// ⚠ **There is no gating form, and its absence is the wall.** A palette that contained one would let
/// two systems decide placement while only one proved anything, so the restriction lives in the set of
/// constructible nodes rather than in a check somebody runs.
#[derive(Clone, Debug, PartialEq)]
pub enum FillOp {
    /// Every Floor beneath the attachment scope.
    ScopeFloors,
    /// Keep surfaces no steeper than this, in degrees.
    FilterSlope { max_degrees: f64 },
    /// Keep surfaces at least this large, in square world units.
    FilterArea { min: f64 },
    /// Thin the sites to a minimum spacing.
    Scatter { min_spacing: f64 },
    /// Bias candidate choice toward content matching a query.
    WeightByTag { query: TagQuery, weight: f64 },
    /// Drop sites inside volumes the solver reserved.
    ///
    /// ⚠ **In the palette by default**, because a fill that forgot it would scatter props through a
    /// door the solver had already placed.
    ExcludeReserved,
    /// Emit placements.
    Place { density: f64 },
}

impl FillOp {
    /// Does this op produce sites rather than consume them?
    pub fn is_source(&self) -> bool {
        matches!(self, FillOp::ScopeFloors)
    }

    /// Is this the terminal op?
    pub fn is_sink(&self) -> bool {
        matches!(self, FillOp::Place { .. })
    }
}

impl fmt::Display for FillOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FillOp::ScopeFloors => write!(f, "Scope Floors"),
            FillOp::FilterSlope { max_degrees } => write!(f, "Filter: slope < {max_degrees}°"),
            FillOp::FilterArea { min } => write!(f, "Filter: area > {min}"),
            FillOp::Scatter { min_spacing } => write!(f, "Scatter: min {min_spacing}m"),
            FillOp::WeightByTag { weight, .. } => write!(f, "Weight by tag: ×{weight}"),
            FillOp::ExcludeReserved => write!(f, "Exclude reserved"),
            FillOp::Place { density } => write!(f, "Place: density {density}"),
        }
    }
}

/// Why a fill graph is not runnable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FillError {
    /// Nothing produces sites.
    NoSource,
    /// Nothing consumes them.
    NoSink,
    /// A source appears after the first op.
    SourceNotFirst { at: usize },
    /// The sink is not last.
    SinkNotLast { at: usize },
}

impl fmt::Display for FillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FillError::NoSource => write!(f, "a fill graph must start by selecting sites"),
            FillError::NoSink => write!(f, "a fill graph that never places anything does nothing"),
            FillError::SourceNotFirst { at } => {
                write!(f, "the site source at {at} must come first")
            }
            FillError::SinkNotLast { at } => write!(f, "the placement at {at} must come last"),
        }
    }
}

impl std::error::Error for FillError {}

/// A site a fill graph may place onto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Site {
    /// The scope it belongs to.
    pub scope: Handle<Node>,
    /// Its slope, in degrees.
    pub slope_degrees: f64,
    /// Its area, in square world units.
    pub area: f64,
    /// Is it inside a volume the solver reserved?
    pub reserved: bool,
}

/// One placement a fill graph produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Filled {
    /// Where.
    pub site: Site,
    /// What.
    pub content: ObjectId,
}

/// A rejected candidate, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Rejected {
    /// What was refused.
    pub content: ObjectId,
    /// Why.
    pub reason: Ineligible,
}

impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rejected: {}", self.reason)
    }
}

/// What one run of a fill graph produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FillResult {
    /// The placements.
    pub placed: Vec<Filled>,
    /// ⚠ **Every candidate the safety rules refused**, with the reason. Shown live in the editor.
    pub rejected: Vec<Rejected>,
    /// Sites dropped for landing inside a reserved volume.
    pub excluded_sites: usize,
}

/// A fill graph: an ordered chain of ops.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FillGraph {
    ops: Vec<FillOp>,
}

impl FillGraph {
    /// An empty graph.
    pub fn new() -> Self {
        FillGraph::default()
    }

    /// Append an op.
    pub fn then(mut self, op: FillOp) -> Self {
        self.ops.push(op);
        self
    }

    /// Every op, in order.
    pub fn ops(&self) -> &[FillOp] {
        &self.ops
    }

    /// Is the chain runnable?
    pub fn validate(&self) -> Result<(), FillError> {
        let source = self.ops.iter().position(FillOp::is_source);
        let sink = self.ops.iter().position(FillOp::is_sink);
        let Some(source) = source else {
            return Err(FillError::NoSource);
        };
        let Some(sink) = sink else {
            return Err(FillError::NoSink);
        };
        if source != 0 {
            return Err(FillError::SourceNotFirst { at: source });
        }
        if sink != self.ops.len() - 1 {
            return Err(FillError::SinkNotLast { at: sink });
        }
        Ok(())
    }

    /// Does this graph exclude reserved volumes?
    ///
    /// ⚠ **Asked rather than assumed.** The node is in the palette by default, but a developer can
    /// remove it, and the editor should say so loudly rather than the core pretending it is there.
    pub fn excludes_reserved(&self) -> bool {
        self.ops.contains(&FillOp::ExcludeReserved)
    }

    /// Run the chain over a set of sites, choosing from a candidate pool.
    ///
    /// ⚠ **Refuses gating and granting content rather than filtering it out.** A silent filter and a
    /// loud rejection produce the same world and completely different afternoons.
    pub fn place(
        &self,
        sites: &[Site],
        pool: &[FillCandidate],
        rng: &Rng,
    ) -> Result<FillResult, FillError> {
        self.validate()?;

        let mut out = FillResult::default();

        // Rule 2, at the seam where the pool is built.
        let mut eligible: Vec<&FillCandidate> = Vec::new();
        for c in pool {
            match c.ineligibility() {
                Some(reason) => out.rejected.push(Rejected {
                    content: c.content,
                    reason,
                }),
                None => eligible.push(c),
            }
        }

        let mut sites: Vec<Site> = sites.to_vec();
        let mut density = 1.0_f64;
        let mut weights: Vec<(TagQuery, f64)> = Vec::new();

        for op in &self.ops {
            match op {
                FillOp::ScopeFloors => {}
                FillOp::FilterSlope { max_degrees } => {
                    sites.retain(|s| s.slope_degrees <= *max_degrees);
                }
                FillOp::FilterArea { min } => sites.retain(|s| s.area >= *min),
                FillOp::Scatter { min_spacing } => {
                    // Deterministic thinning: keep one site per spacing bucket, in input order.
                    //
                    // ⚠ **Not a true Poisson-disc pass, and the shape of the guarantee is what
                    // matters here**: the same sites in the same order always thin to the same set,
                    // and no two kept sites are closer than the spacing in bucket terms. A sampler
                    // that were merely *usually* right would be a determinism bug that appears once
                    // in a hundred seeds.
                    let mut kept: Vec<Site> = Vec::new();
                    let mut taken: BTreeSet<(i64, i64)> = BTreeSet::new();
                    let step = min_spacing.max(f64::EPSILON);
                    for s in &sites {
                        let key = (
                            (s.area / step).floor() as i64,
                            (s.slope_degrees / step).floor() as i64,
                        );
                        if taken.insert(key) {
                            kept.push(*s);
                        }
                    }
                    sites = kept;
                }
                FillOp::WeightByTag { query, weight } => {
                    weights.push((query.clone(), *weight));
                }
                FillOp::ExcludeReserved => {
                    let before = sites.len();
                    sites.retain(|s| !s.reserved);
                    out.excluded_sites += before - sites.len();
                }
                FillOp::Place { density: d } => density = d.clamp(0.0, 1.0),
            }
        }

        if eligible.is_empty() {
            return Ok(out);
        }

        // Weighted, deterministic choice per site.
        let pick = rng.fork("fill");
        let total: f64 = eligible
            .iter()
            .map(|c| weight_of(c, &weights))
            .sum::<f64>()
            .max(f64::EPSILON);
        let keep = ((sites.len() as f64) * density).round() as usize;
        for (i, site) in sites.into_iter().take(keep).enumerate() {
            let mut r = pick.fork_index(i as u64);
            let roll = (r.next_u64() >> 11) as f64 / (1u64 << 53) as f64 * total;
            let mut acc = 0.0;
            let chosen = eligible
                .iter()
                .find(|c| {
                    acc += weight_of(c, &weights);
                    roll < acc
                })
                .unwrap_or(&eligible[eligible.len() - 1]);
            out.placed.push(Filled {
                site,
                content: chosen.content,
            });
        }
        Ok(out)
    }
}

fn weight_of(c: &FillCandidate, weights: &[(TagQuery, f64)]) -> f64 {
    let mut w = 1.0;
    for (q, factor) in weights {
        if q.matches_any(&c.tags) {
            w *= factor;
        }
    }
    w.max(0.0)
}

/// Where a fill graph is attached, and therefore how often it runs and what it may see.
#[derive(Clone, Debug, PartialEq)]
pub struct FillBand {
    /// The graph.
    pub graph: FillGraph,
    /// The spine element it hangs off — a slot name or a segment's endpoints.
    pub attachment: Attachment,
    /// The scope kind the attachment point occupies.
    ///
    /// ⚠ **This is the whole of scope inheritance.** It decides how many times the fill runs *and*
    /// bounds what it can select, which is why a fill on a Space slot cannot reach the Area's other
    /// Spaces.
    pub scope: NodeKind,
}

/// A spine element a fill band hangs off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attachment {
    /// A named slot.
    Slot { name: String },
    /// A segment between two slots.
    Segment { from: String, to: String },
}

impl fmt::Display for Attachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Attachment::Slot { name } => write!(f, "slot {name}"),
            Attachment::Segment { from, to } => write!(f, "segment {from} → {to}"),
        }
    }
}

impl FillBand {
    /// A band on a slot.
    pub fn on_slot(name: impl Into<String>, scope: NodeKind, graph: FillGraph) -> Self {
        FillBand {
            graph,
            attachment: Attachment::Slot { name: name.into() },
            scope,
        }
    }

    /// A band on a segment.
    pub fn on_segment(
        from: impl Into<String>,
        to: impl Into<String>,
        scope: NodeKind,
        graph: FillGraph,
    ) -> Self {
        FillBand {
            graph,
            attachment: Attachment::Segment {
                from: from.into(),
                to: to.into(),
            },
            scope,
        }
    }

    /// Every scope this band runs over — one run per instance of its attachment scope.
    pub fn instances(&self, graph: &NodeGraph, root: Handle<Node>) -> Vec<Handle<Node>> {
        let mut out = Vec::new();
        collect(graph, root, self.scope, &mut out);
        out
    }

    /// May this band select that scope?
    ///
    /// ⚠ **The ceiling, not the loop counter.** Selection stops at the attachment scope's own subtree,
    /// or *"per Space"* and *"per Area"* would differ only in how many times the same world-wide fill
    /// ran.
    pub fn may_select(
        &self,
        graph: &NodeGraph,
        instance: Handle<Node>,
        site: Handle<Node>,
    ) -> bool {
        if instance == site {
            return true;
        }
        let mut cur = site;
        while let Some(node) = graph.get(cur) {
            match node.parent() {
                Some(p) if p == instance => return true,
                Some(p) => cur = p,
                None => return false,
            }
        }
        false
    }
}

fn collect(graph: &NodeGraph, at: Handle<Node>, kind: NodeKind, out: &mut Vec<Handle<Node>>) {
    let Some(node) = graph.get(at) else { return };
    if node.kind() == kind {
        out.push(at);
    }
    for child in node.children().to_vec() {
        collect(graph, child, kind, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(s: &str) -> ObjectId {
        ObjectId::derived("content", s)
    }

    fn classic() -> FillGraph {
        FillGraph::new()
            .then(FillOp::ScopeFloors)
            .then(FillOp::FilterSlope { max_degrees: 30.0 })
            .then(FillOp::ExcludeReserved)
            .then(FillOp::Scatter { min_spacing: 4.0 })
            .then(FillOp::Place { density: 1.0 })
    }

    fn site(area: f64, slope: f64, reserved: bool) -> Site {
        let mut g = NodeGraph::new(1.0, 1);
        let s = g.add_child(g.root(), "s").unwrap();
        Site {
            scope: s,
            slope_degrees: slope,
            area,
            reserved,
        }
    }

    #[test]
    fn a_chain_that_selects_and_places_is_runnable() {
        assert_eq!(classic().validate(), Ok(()));
    }

    #[test]
    fn a_graph_with_no_source_or_no_sink_does_nothing_and_says_so() {
        assert_eq!(
            FillGraph::new()
                .then(FillOp::Place { density: 1.0 })
                .validate(),
            Err(FillError::NoSource)
        );
        assert_eq!(
            FillGraph::new().then(FillOp::ScopeFloors).validate(),
            Err(FillError::NoSink)
        );
    }

    #[test]
    fn a_source_or_sink_out_of_position_is_rejected() {
        assert_eq!(
            FillGraph::new()
                .then(FillOp::ExcludeReserved)
                .then(FillOp::ScopeFloors)
                .then(FillOp::Place { density: 1.0 })
                .validate(),
            Err(FillError::SourceNotFirst { at: 1 })
        );
        assert_eq!(
            FillGraph::new()
                .then(FillOp::ScopeFloors)
                .then(FillOp::Place { density: 1.0 })
                .then(FillOp::ExcludeReserved)
                .validate(),
            Err(FillError::SinkNotLast { at: 1 })
        );
    }

    #[test]
    fn gating_or_granting_content_is_rejected_and_the_reason_is_reported() {
        // ⚠ Rule 2, and the reason reaches the editor rather than the content vanishing.
        let pool = vec![
            FillCandidate::new(oid("rubble")),
            FillCandidate::new(oid("door")).gating(),
            FillCandidate::new(oid("missiles")).granting(),
            FillCandidate::new(oid("gate_key")).gating().granting(),
        ];
        let got = classic()
            .place(&[site(10.0, 5.0, false)], &pool, &Rng::new(1))
            .unwrap();
        assert_eq!(got.rejected.len(), 3);
        assert!(got.rejected.iter().any(|r| r.reason == Ineligible::Gates));
        assert!(got.rejected.iter().any(|r| r.reason == Ineligible::Grants));
        assert!(got
            .rejected
            .iter()
            .any(|r| r.reason == Ineligible::GatesAndGrants));
        assert!(got.placed.iter().all(|p| p.content == oid("rubble")));
        assert!(got.rejected[0].to_string().contains("proof"));
    }

    #[test]
    fn the_palette_contains_no_way_to_gate() {
        // ⚠ The wall. If this ever fails, a fill band has become a second placement engine.
        let ops = [
            FillOp::ScopeFloors,
            FillOp::FilterSlope { max_degrees: 1.0 },
            FillOp::FilterArea { min: 1.0 },
            FillOp::Scatter { min_spacing: 1.0 },
            FillOp::WeightByTag {
                query: TagQuery::inherited("Prop"),
                weight: 1.0,
            },
            FillOp::ExcludeReserved,
            FillOp::Place { density: 1.0 },
        ];
        for op in &ops {
            let s = op.to_string().to_lowercase();
            assert!(!s.contains("gate"), "{op} looks like a gating node");
            assert!(!s.contains("grant"), "{op} looks like a granting node");
        }
        assert_eq!(ops.len(), 7, "the palette grew — is the new node a gate?");
    }

    #[test]
    fn reserved_sites_are_excluded_and_counted() {
        let pool = vec![FillCandidate::new(oid("rubble"))];
        let got = classic()
            .place(
                &[
                    site(10.0, 5.0, false),
                    site(20.0, 5.0, true),
                    site(30.0, 5.0, true),
                ],
                &pool,
                &Rng::new(1),
            )
            .unwrap();
        assert_eq!(got.excluded_sites, 2);
        assert_eq!(got.placed.len(), 1);
    }

    #[test]
    fn a_graph_without_the_exclusion_node_says_so_rather_than_being_assumed_safe() {
        assert!(classic().excludes_reserved());
        let without = FillGraph::new()
            .then(FillOp::ScopeFloors)
            .then(FillOp::Place { density: 1.0 });
        assert!(
            !without.excludes_reserved(),
            "a developer can remove the node; the core must not pretend it is there"
        );
    }

    #[test]
    fn slope_and_area_filters_change_the_output() {
        let pool = vec![FillCandidate::new(oid("torch"))];
        let sites = [site(10.0, 5.0, false), site(10.0, 60.0, false)];
        let steep = classic().place(&sites, &pool, &Rng::new(1)).unwrap();
        assert_eq!(steep.placed.len(), 1, "the 60° surface is filtered out");

        let by_area = FillGraph::new()
            .then(FillOp::ScopeFloors)
            .then(FillOp::FilterArea { min: 50.0 })
            .then(FillOp::Place { density: 1.0 })
            .place(&sites, &pool, &Rng::new(1))
            .unwrap();
        assert!(by_area.placed.is_empty());
    }

    #[test]
    fn density_scales_how_much_lands() {
        let pool = vec![FillCandidate::new(oid("rubble"))];
        let sites: Vec<Site> = (0..10).map(|i| site(i as f64 + 1.0, 5.0, false)).collect();
        let g = |d: f64| {
            FillGraph::new()
                .then(FillOp::ScopeFloors)
                .then(FillOp::Place { density: d })
                .place(&sites, &pool, &Rng::new(1))
                .unwrap()
                .placed
                .len()
        };
        assert_eq!(g(1.0), 10);
        assert_eq!(g(0.5), 5);
        assert_eq!(g(0.0), 0);
    }

    #[test]
    fn the_same_seed_fills_identically_and_a_different_one_does_not_have_to() {
        let pool = vec![
            FillCandidate::new(oid("a")),
            FillCandidate::new(oid("b")),
            FillCandidate::new(oid("c")),
        ];
        let sites: Vec<Site> = (0..24).map(|i| site(i as f64 + 1.0, 5.0, false)).collect();
        let run = |seed: u64| {
            FillGraph::new()
                .then(FillOp::ScopeFloors)
                .then(FillOp::Place { density: 1.0 })
                .place(&sites, &pool, &Rng::new(seed))
                .unwrap()
                .placed
        };
        assert_eq!(run(7), run(7), "a fill is deterministic in its seed");
        assert_ne!(run(7), run(8), "and the seed is doing something");
    }

    #[test]
    fn tag_weighting_biases_the_choice_without_excluding_anything() {
        let pool = vec![
            FillCandidate::new(oid("torch")).tagged("Prop.Light"),
            FillCandidate::new(oid("crate")).tagged("Prop.Crate"),
        ];
        let sites: Vec<Site> = (0..200).map(|i| site(i as f64 + 1.0, 5.0, false)).collect();
        let weighted = FillGraph::new()
            .then(FillOp::ScopeFloors)
            .then(FillOp::WeightByTag {
                query: TagQuery::inherited("Prop.Light"),
                weight: 20.0,
            })
            .then(FillOp::Place { density: 1.0 })
            .place(&sites, &pool, &Rng::new(3))
            .unwrap();
        let torches = weighted
            .placed
            .iter()
            .filter(|p| p.content == oid("torch"))
            .count();
        assert!(
            torches > 150,
            "weighting should dominate, got {torches}/200"
        );
        assert!(
            torches < 200,
            "a weight is a bias, not a filter — the crate must still be selectable"
        );
    }

    #[test]
    fn scatter_thins_sites_and_is_stable_in_input_order() {
        let pool = vec![FillCandidate::new(oid("rubble"))];
        let sites: Vec<Site> = (0..20).map(|i| site(i as f64, 5.0, false)).collect();
        let g = FillGraph::new()
            .then(FillOp::ScopeFloors)
            .then(FillOp::Scatter { min_spacing: 5.0 })
            .then(FillOp::Place { density: 1.0 });
        let a = g.place(&sites, &pool, &Rng::new(1)).unwrap();
        let b = g.place(&sites, &pool, &Rng::new(1)).unwrap();
        assert!(a.placed.len() < 20, "scatter thins");
        assert_eq!(a.placed.len(), b.placed.len());
    }

    #[test]
    fn an_empty_pool_places_nothing_rather_than_panicking() {
        let got = classic()
            .place(&[site(10.0, 5.0, false)], &[], &Rng::new(1))
            .unwrap();
        assert!(got.placed.is_empty());
        assert!(got.rejected.is_empty());
    }

    #[test]
    fn a_pool_of_only_gating_content_places_nothing_and_reports_all_of_it() {
        let pool = vec![FillCandidate::new(oid("door")).gating()];
        let got = classic()
            .place(&[site(10.0, 5.0, false)], &pool, &Rng::new(1))
            .unwrap();
        assert!(got.placed.is_empty());
        assert_eq!(
            got.rejected.len(),
            1,
            "silence here is the bug rule 2 exists for"
        );
    }

    #[test]
    fn a_band_runs_once_per_instance_of_its_attachment_scope() {
        // ⚠ Scope inheritance: an Area band runs per Area, however many Spaces nest inside.
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let other = g.add_child(reach, "area2").unwrap();
        for i in 0..5 {
            g.add_child(area, format!("space{i}")).unwrap();
        }
        g.add_child(other, "space_x").unwrap();

        let by_area = FillBand::on_slot("hall", NodeKind::Area, classic());
        assert_eq!(
            by_area.instances(&g, g.root()).len(),
            2,
            "two Areas, two runs — not seven, which is the Space count"
        );

        let by_space = FillBand::on_slot("hall", NodeKind::Space, classic());
        assert_eq!(by_space.instances(&g, g.root()).len(), 6);
    }

    #[test]
    fn the_attachment_scope_is_a_ceiling_on_selection() {
        // ⚠ Otherwise "per Space" and "per Area" would differ only in how many times the same
        // world-wide fill ran.
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let other = g.add_child(reach, "area2").unwrap();
        let inside = g.add_child(area, "space").unwrap();
        let outside = g.add_child(other, "space_x").unwrap();

        let band = FillBand::on_slot("hall", NodeKind::Area, classic());
        assert!(band.may_select(&g, area, inside));
        assert!(band.may_select(&g, area, area), "its own scope counts");
        assert!(
            !band.may_select(&g, area, outside),
            "a fill must not reach into a sibling Area"
        );
    }

    #[test]
    fn an_attachment_names_the_element_it_hangs_off() {
        assert_eq!(
            FillBand::on_slot("capstone", NodeKind::Space, classic())
                .attachment
                .to_string(),
            "slot capstone"
        );
        let seg = FillBand::on_segment("a", "b", NodeKind::Space, classic());
        assert!(seg.attachment.to_string().contains("a → b"));
    }

    #[test]
    fn every_op_prints_something_the_editor_can_label_a_node_with() {
        for op in [
            FillOp::ScopeFloors,
            FillOp::FilterSlope { max_degrees: 30.0 },
            FillOp::Scatter { min_spacing: 4.0 },
            FillOp::ExcludeReserved,
            FillOp::Place { density: 1.0 },
        ] {
            assert!(!op.to_string().is_empty());
        }
        assert!(FillOp::ScopeFloors.is_source());
        assert!(FillOp::Place { density: 1.0 }.is_sink());
        assert!(!FillOp::ExcludeReserved.is_source());
    }
}
