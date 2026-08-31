//! The scope graph — `World → Reach → Area → Space → Spatial` — and the lazy-generation lifecycle
//! that governs when each part of it becomes real.
//!
//! # Algorithm-owned, script-readable
//!
//! Nodes are *structure*, and structure belongs to the algorithm. A script may read a node freely but
//! never write one; changes arrive as `Context` requests the core may grant, adapt, or deny. That
//! is enforced here rather than merely documented: [`Node`] exposes **getters only**, and every mutation
//! lives on [`NodeGraph`] where the invariants can be checked. There is no way to obtain a
//! `&mut Node` — a realized node is immutable because no API can hand you one.
//!
//! # The lifecycle, and the one invariant that matters
//!
//! Lazy generation means the world is *forecast* long before it is *built*: distant regions stay
//! abstract until something needs them. Each node walks a one-way path:
//!
//! ```text
//! Projected  ──▶  Reserved  ──▶  Realized
//! (a revisable   (committed to   (built and
//!  forecast)      exist; space    frozen)
//!                 claimed)
//! ```
//!
//! The invariant tying it together is:
//!
//! > **A node's state may never exceed its parent's.**
//!
//! That single rule expresses lazy generation exactly. A subtree may lag arbitrarily far behind — an
//! untouched Reach can sit `Projected` while another is fully `Realized` — but it can never lead,
//! because realizing a Space inside a merely-imagined Area is incoherent: the Area's envelope is not
//! fixed yet, so the Space has nothing to be placed *within*.
//!
//! Two consequences fall out, both enforced:
//!
//! * **Progress is monotone.** There is no demotion, so a fact committed at `Reserved` is never
//!   retracted — the "solvability-monotone revision" property the pipeline depends on.
//! * **Only forecasts can be discarded.** A node may be removed while `Projected`; once `Reserved`,
//!   something is counting on it existing.
//!
//! # Determinism
//!
//! Children and neighbours are kept in insertion order (never sorted by handle or hashed), traversal is
//! depth-first in that order, and every query returns a deterministic sequence. Given the same
//! operations, the same graph — on every run and every target.

use crate::arena::{Arena, Handle};
use crate::object::{IdAllocator, Object, ObjectHeader};
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use cv_determinism::Aabb;
use std::fmt;

/// Where a node sits in the containment hierarchy.
///
/// The order is fixed and total: each kind contains exactly one kind, which is what makes the
/// hierarchy checkable rather than conventional.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    /// The singleton root: the whole generated world.
    World,
    /// A major progression region — the granularity lazy generation streams at.
    Reach,
    /// A themed sub-region within a Reach.
    ///
    /// The scope a theme attaches *to*. Themeing itself is not a kind of content: it is dial values
    /// on an Area-scoped spine slot, which is why there is no `Biome` anywhere in this tree.
    Area,
    /// A room: the unit most puzzles and encounters are scoped to.
    Space,
    /// A sub-volume within a Space (a ledge, an alcove, a shaft).
    Spatial,
}

impl NodeKind {
    /// Every kind, outermost first.
    pub const ALL: [NodeKind; 5] = [
        NodeKind::World,
        NodeKind::Reach,
        NodeKind::Area,
        NodeKind::Space,
        NodeKind::Spatial,
    ];

    /// The only kind this one may contain, or `None` for the innermost.
    pub fn child_kind(self) -> Option<NodeKind> {
        match self {
            NodeKind::World => Some(NodeKind::Reach),
            NodeKind::Reach => Some(NodeKind::Area),
            NodeKind::Area => Some(NodeKind::Space),
            NodeKind::Space => Some(NodeKind::Spatial),
            NodeKind::Spatial => None,
        }
    }

    /// Depth from the root: `World` is 0, `Spatial` is 4.
    pub fn depth(self) -> u8 {
        match self {
            NodeKind::World => 0,
            NodeKind::Reach => 1,
            NodeKind::Area => 2,
            NodeKind::Space => 3,
            NodeKind::Spatial => 4,
        }
    }

    /// The name used in diagnostics and the editor.
    pub fn as_str(self) -> &'static str {
        match self {
            NodeKind::World => "World",
            NodeKind::Reach => "Reach",
            NodeKind::Area => "Area",
            NodeKind::Space => "Space",
            NodeKind::Spatial => "Spatial",
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        NodeKind::ALL.get(tag as usize).copied()
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How far along the lazy-generation lifecycle a node is.
///
/// Ordered, and the ordering is load-bearing: comparisons enforce the parent/child invariant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeState {
    /// A revisable forecast. Mutable, and the only state from which a node may be removed.
    Projected,
    /// Committed to exist, with an envelope claimed. Details may still be refined; it cannot be
    /// removed, reparented, or have its kind changed.
    Reserved,
    /// Built and frozen. No mutation of any kind.
    Realized,
}

impl NodeState {
    /// All states, in lifecycle order.
    pub const ALL: [NodeState; 3] = [
        NodeState::Projected,
        NodeState::Reserved,
        NodeState::Realized,
    ];

    /// The name used in diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            NodeState::Projected => "Projected",
            NodeState::Reserved => "Reserved",
            NodeState::Realized => "Realized",
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        NodeState::ALL.get(tag as usize).copied()
    }

    fn tag(self) -> u8 {
        match self {
            NodeState::Projected => 0,
            NodeState::Reserved => 1,
            NodeState::Realized => 2,
        }
    }
}

impl fmt::Display for NodeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Everything that can go wrong while shaping the graph.
///
/// Every one is a *rejected* operation, never a corrupted graph: an `Err` means nothing changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeError {
    /// The handle is stale or was never issued by this graph.
    StaleHandle,
    /// The node is `Realized` and therefore frozen.
    Immutable { kind: NodeKind, state: NodeState },
    // NB: there is deliberately no `IllegalContainment` variant. `add_child` *derives* the child's
    // kind from the parent's, so the hierarchy cannot be violated through the API at all — an error
    // for it would be unconstructible. Violations only arise in a corrupt stream, where
    // `check_invariants` catches them at load.
    /// The innermost kind cannot contain anything.
    NotAContainer { kind: NodeKind },
    /// States only move forward.
    Regression { from: NodeState, to: NodeState },
    /// Advancing would put the node ahead of its parent.
    AheadOfParent {
        parent: NodeState,
        requested: NodeState,
    },
    /// A node cannot be `Reserved` without an envelope to reserve.
    MissingEnvelope { kind: NodeKind },
    /// Only forecasts may be discarded.
    NotProjected { state: NodeState },
    /// A descendant has advanced past `Projected`, so the subtree is committed.
    SubtreeCommitted { descendant_state: NodeState },
    /// Adjacency links peers; these are different kinds.
    KindMismatch { a: NodeKind, b: NodeKind },
    /// A node cannot be adjacent to itself.
    SelfAdjacency,
    /// The root has no parent and cannot be removed or reparented.
    RootNode,
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeError::StaleHandle => write!(f, "stale node handle"),
            NodeError::Immutable { kind, state } => {
                write!(f, "{kind} is {state} and cannot be modified")
            }
            NodeError::NotAContainer { kind } => write!(f, "a {kind} cannot contain child nodes"),
            NodeError::Regression { from, to } => {
                write!(
                    f,
                    "cannot move from {from} back to {to}; the lifecycle is one-way"
                )
            }
            NodeError::AheadOfParent { parent, requested } => write!(
                f,
                "cannot advance to {requested} while the parent is only {parent} — \
                 a child may lag behind its parent but never lead it"
            ),
            NodeError::MissingEnvelope { kind } => {
                write!(f, "{kind} needs an envelope before it can be reserved")
            }
            NodeError::NotProjected { state } => {
                write!(
                    f,
                    "only Projected nodes may be removed; this one is {state}"
                )
            }
            NodeError::SubtreeCommitted { descendant_state } => write!(
                f,
                "a descendant is {descendant_state}; the subtree is committed and cannot be removed"
            ),
            NodeError::KindMismatch { a, b } => {
                write!(f, "adjacency links peers, but these are a {a} and a {b}")
            }
            NodeError::SelfAdjacency => write!(f, "a node cannot be adjacent to itself"),
            NodeError::RootNode => write!(f, "the World root has no parent"),
        }
    }
}

impl std::error::Error for NodeError {}

/// Result alias for graph operations.
pub type NodeResult<T> = Result<T, NodeError>;

/// One scope in the hierarchy.
///
/// **Read-only by construction.** Fields are private and there are no setters; every mutation goes
/// through [`NodeGraph`], which is where the lifecycle and containment rules can actually be checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    header: ObjectHeader,
    kind: NodeKind,
    state: NodeState,
    parent: Option<Handle<Node>>,
    children: Vec<Handle<Node>>,
    /// Peers this node connects to (Space↔Space map edges). Undirected: both sides record the link.
    neighbors: Vec<Handle<Node>>,
    /// The spatial bounds this node claims. `None` until a reservation fixes it.
    envelope: Option<Aabb>,
}

impl Node {
    /// Where this node sits in the hierarchy.
    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    /// How far along the lifecycle this node is.
    pub fn state(&self) -> NodeState {
        self.state
    }

    /// The containing scope, or `None` for the World root.
    pub fn parent(&self) -> Option<Handle<Node>> {
        self.parent
    }

    /// Contained scopes, in insertion order.
    pub fn children(&self) -> &[Handle<Node>] {
        &self.children
    }

    /// Adjacent peers, in insertion order.
    pub fn neighbors(&self) -> &[Handle<Node>] {
        &self.neighbors
    }

    /// The claimed spatial bounds, if any.
    pub fn envelope(&self) -> Option<Aabb> {
        self.envelope
    }

    /// Is this node frozen?
    pub fn is_realized(&self) -> bool {
        self.state == NodeState::Realized
    }
}

impl Object for Node {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut ObjectHeader {
        &mut self.header
    }
    fn type_name(&self) -> &'static str {
        self.kind.as_str()
    }
}

/// The scope graph: containment tree plus peer adjacency, rooted at the World node.
///
/// This is *structure*. The mission graph (progression edges, gates, traversal requirements) is a
/// separate layer built on top at M09 — adjacency here means "these scopes are spatially connected",
/// not "you can get from one to the other".
#[derive(Clone, Debug, PartialEq)]
pub struct NodeGraph {
    nodes: Arena<Node>,
    root: Handle<Node>,
    ids: IdAllocator,
    /// World units per metre.
    scale: f64,
    /// The seed this world was generated from — a runtime input, never part of the fingerprint.
    seed: u64,
}

impl NodeGraph {
    /// A new graph containing only a `Projected` World root.
    pub fn new(scale: f64, seed: u64) -> Self {
        let mut nodes = Arena::new();
        let mut ids = IdAllocator::new();
        let root = nodes.insert(Node {
            header: ObjectHeader::new(ids.allocate(), "World"),
            kind: NodeKind::World,
            state: NodeState::Projected,
            parent: None,
            children: Vec::new(),
            neighbors: Vec::new(),
            envelope: None,
        });
        NodeGraph {
            nodes,
            root,
            ids,
            scale,
            seed,
        }
    }

    /// The World root.
    pub fn root(&self) -> Handle<Node> {
        self.root
    }

    /// World units per metre.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// The seed this world was generated from.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// How many nodes exist.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Is the graph empty? Never true — a graph always has its World root.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // --- reads -------------------------------------------------------------------------------

    /// Borrow a node, or `None` if the handle is stale.
    pub fn get(&self, h: Handle<Node>) -> Option<&Node> {
        self.nodes.get(h)
    }

    /// Borrow a node, erroring on a stale handle.
    pub fn node(&self, h: Handle<Node>) -> NodeResult<&Node> {
        self.nodes.get(h).ok_or(NodeError::StaleHandle)
    }

    /// Every live node, in arena order.
    pub fn iter(&self) -> impl Iterator<Item = (Handle<Node>, &Node)> + '_ {
        self.nodes.iter()
    }

    // --- structure ---------------------------------------------------------------------------

    /// Add a child under `parent`, with the kind the hierarchy dictates.
    ///
    /// The new node starts `Projected` with no envelope. Its kind is *derived*, never passed in, so an
    /// Area can only ever hold Spaces.
    ///
    /// # Realized parents may still gain children
    ///
    /// This is deliberate, and it is what "Realized" actually means. Freezing a node freezes **its own
    /// attributes** — name, envelope, adjacency — not its membership. A realized World must still be
    /// able to accept a Reach that gets streamed in later, or lazy generation could not exist at the
    /// top level at all: the World would have to stay a forecast forever.
    ///
    /// Nothing is weakened by this, because the new child arrives `Projected` and the parent's own
    /// committed data is untouched. Contrast [`NodeGraph::connect`], which *is* frozen at `Realized` —
    /// adjacency is decided during projection, so a realized node's links are already settled.
    pub fn add_child(
        &mut self,
        parent: Handle<Node>,
        name: impl Into<String>,
    ) -> NodeResult<Handle<Node>> {
        let parent_node = self.node(parent)?;
        let kind = parent_node
            .kind
            .child_kind()
            .ok_or(NodeError::NotAContainer {
                kind: parent_node.kind,
            })?;

        let child = self.nodes.insert(Node {
            header: ObjectHeader::new(self.ids.allocate(), name),
            kind,
            state: NodeState::Projected,
            parent: Some(parent),
            children: Vec::new(),
            neighbors: Vec::new(),
            envelope: None,
        });
        self.nodes[parent].children.push(child);
        Ok(child)
    }

    /// Remove a `Projected` node and its (necessarily `Projected`) subtree.
    ///
    /// Rejected if the node or any descendant has advanced — once something is `Reserved`, the rest of
    /// the generator is entitled to assume it exists.
    pub fn remove(&mut self, h: Handle<Node>) -> NodeResult<()> {
        if h == self.root {
            return Err(NodeError::RootNode);
        }
        let node = self.node(h)?;
        if node.state != NodeState::Projected {
            return Err(NodeError::NotProjected { state: node.state });
        }
        // The whole subtree must be discardable, or removing it would strand a commitment.
        let subtree = self.descendants_of(h);
        for d in &subtree {
            let state = self.nodes[*d].state;
            if state != NodeState::Projected {
                return Err(NodeError::SubtreeCommitted {
                    descendant_state: state,
                });
            }
        }

        // Detach from the parent, then unlink and drop the subtree bottom-up.
        if let Some(p) = self.nodes[h].parent {
            self.nodes[p].children.retain(|c| *c != h);
        }
        for d in subtree.into_iter().rev() {
            self.unlink_all(d);
            self.nodes.remove(d);
        }
        self.unlink_all(h);
        self.nodes.remove(h);
        Ok(())
    }

    /// Link two peers as spatially adjacent. Undirected and idempotent.
    ///
    /// Both must be the same kind: adjacency connects rooms to rooms, not a room to a region.
    pub fn connect(&mut self, a: Handle<Node>, b: Handle<Node>) -> NodeResult<()> {
        if a == b {
            return Err(NodeError::SelfAdjacency);
        }
        let (ka, sa) = {
            let n = self.node(a)?;
            (n.kind, n.state)
        };
        let (kb, sb) = {
            let n = self.node(b)?;
            (n.kind, n.state)
        };
        if ka != kb {
            return Err(NodeError::KindMismatch { a: ka, b: kb });
        }
        if sa == NodeState::Realized {
            return Err(NodeError::Immutable {
                kind: ka,
                state: sa,
            });
        }
        if sb == NodeState::Realized {
            return Err(NodeError::Immutable {
                kind: kb,
                state: sb,
            });
        }
        if !self.nodes[a].neighbors.contains(&b) {
            self.nodes[a].neighbors.push(b);
        }
        if !self.nodes[b].neighbors.contains(&a) {
            self.nodes[b].neighbors.push(a);
        }
        Ok(())
    }

    /// Remove an adjacency link. Idempotent.
    pub fn disconnect(&mut self, a: Handle<Node>, b: Handle<Node>) -> NodeResult<()> {
        let (ka, sa) = {
            let n = self.node(a)?;
            (n.kind, n.state)
        };
        let (kb, sb) = {
            let n = self.node(b)?;
            (n.kind, n.state)
        };
        if sa == NodeState::Realized {
            return Err(NodeError::Immutable {
                kind: ka,
                state: sa,
            });
        }
        if sb == NodeState::Realized {
            return Err(NodeError::Immutable {
                kind: kb,
                state: sb,
            });
        }
        self.nodes[a].neighbors.retain(|n| *n != b);
        self.nodes[b].neighbors.retain(|n| *n != a);
        Ok(())
    }

    // --- mutation (the complete set; there is no `&mut Node`) ---------------------------------

    /// Rename a node. Names are for humans; identity is unaffected.
    pub fn set_name(&mut self, h: Handle<Node>, name: impl Into<String>) -> NodeResult<()> {
        self.check_mutable(self.node(h)?)?;
        self.nodes[h].header.name = name.into();
        Ok(())
    }

    /// Claim or refine this node's spatial bounds. Allowed while `Projected` or `Reserved`.
    pub fn set_envelope(&mut self, h: Handle<Node>, envelope: Aabb) -> NodeResult<()> {
        self.check_mutable(self.node(h)?)?;
        self.nodes[h].envelope = Some(envelope);
        Ok(())
    }

    /// Advance a node along the lifecycle.
    ///
    /// Enforces all three rules at once: no regression, never ahead of the parent, and no reservation
    /// without an envelope.
    pub fn advance(&mut self, h: Handle<Node>, to: NodeState) -> NodeResult<()> {
        let node = self.node(h)?;
        let from = node.state;
        if to < from {
            return Err(NodeError::Regression { from, to });
        }
        if to == from {
            return Ok(()); // idempotent
        }
        if node.envelope.is_none() {
            return Err(NodeError::MissingEnvelope { kind: node.kind });
        }
        if let Some(p) = node.parent {
            let parent_state = self.node(p)?.state;
            if to > parent_state {
                return Err(NodeError::AheadOfParent {
                    parent: parent_state,
                    requested: to,
                });
            }
        }
        self.nodes[h].state = to;
        Ok(())
    }

    /// Advance a node and every ancestor up to the root to at least `to`.
    ///
    /// The usual way to realize something on demand: rather than making callers walk the chain by hand
    /// to satisfy the parent invariant, this establishes it top-down.
    pub fn advance_with_ancestors(&mut self, h: Handle<Node>, to: NodeState) -> NodeResult<()> {
        let mut chain = self.ancestors_of(h);
        chain.reverse(); // root first, so each parent advances before its child
        chain.push(h);
        for node in chain {
            if self.node(node)?.state < to {
                self.advance(node, to)?;
            }
        }
        Ok(())
    }

    // --- queries -----------------------------------------------------------------------------

    /// This node's ancestors, nearest first, ending at the root.
    pub fn ancestors_of(&self, h: Handle<Node>) -> Vec<Handle<Node>> {
        let mut out = Vec::new();
        let mut at = self.get(h).and_then(|n| n.parent);
        while let Some(cur) = at {
            out.push(cur);
            at = self.get(cur).and_then(|n| n.parent);
        }
        out
    }

    /// Every descendant, depth-first in child order (not including `h` itself).
    pub fn descendants_of(&self, h: Handle<Node>) -> Vec<Handle<Node>> {
        let mut out = Vec::new();
        let mut stack: Vec<Handle<Node>> = match self.get(h) {
            Some(n) => n.children.iter().rev().copied().collect(),
            None => return out,
        };
        while let Some(cur) = stack.pop() {
            out.push(cur);
            if let Some(n) = self.get(cur) {
                stack.extend(n.children.iter().rev().copied());
            }
        }
        out
    }

    /// The enclosing scope of a given kind, or `None` if there is none.
    ///
    /// Answers "which Space is this ledge in?" without the caller walking parents by hand.
    pub fn scope_of(&self, h: Handle<Node>, kind: NodeKind) -> Option<Handle<Node>> {
        let node = self.get(h)?;
        if node.kind == kind {
            return Some(h);
        }
        self.ancestors_of(h)
            .into_iter()
            .find(|a| self.nodes[*a].kind == kind)
    }

    /// Depth below the root; the root is 0.
    pub fn depth_of(&self, h: Handle<Node>) -> usize {
        self.ancestors_of(h).len()
    }

    /// Every node of a kind, in arena order.
    pub fn of_kind(&self, kind: NodeKind) -> impl Iterator<Item = (Handle<Node>, &Node)> + '_ {
        self.nodes.iter().filter(move |(_, n)| n.kind == kind)
    }

    /// Every node satisfying `pred`, in arena order.
    pub fn find<'a>(
        &'a self,
        pred: impl Fn(&Node) -> bool + 'a,
    ) -> impl Iterator<Item = (Handle<Node>, &'a Node)> + 'a {
        self.nodes.iter().filter(move |(_, n)| pred(n))
    }

    /// The whole tree in depth-first order, root first — the canonical deterministic walk.
    pub fn walk(&self) -> Vec<Handle<Node>> {
        let mut out = vec![self.root];
        out.extend(self.descendants_of(self.root));
        out
    }

    // --- internals ---------------------------------------------------------------------------

    /// Reject mutation of a realized node.
    fn check_mutable(&self, node: &Node) -> NodeResult<()> {
        if node.state == NodeState::Realized {
            return Err(NodeError::Immutable {
                kind: node.kind,
                state: node.state,
            });
        }
        Ok(())
    }

    /// Drop every adjacency referencing `h`, so no dangling links survive its removal.
    fn unlink_all(&mut self, h: Handle<Node>) {
        let peers = std::mem::take(&mut self.nodes[h].neighbors);
        for p in peers {
            if let Some(peer) = self.nodes.get_mut(p) {
                peer.neighbors.retain(|n| *n != h);
            }
        }
    }

    /// Verify every structural invariant. Used by tests and available to the editor's diagnostics.
    ///
    /// Returns a description of the first violation found, or `None` when the graph is sound.
    pub fn check_invariants(&self) -> Option<String> {
        for (h, node) in self.nodes.iter() {
            // Parent links are symmetric with child lists.
            match node.parent {
                Some(p) => {
                    // NB: not `self.get(p)?` — `?` on an Option<String> return would treat a dangling
                    // handle as "no violation found", which is the opposite of the truth.
                    let Some(parent) = self.get(p) else {
                        return Some(format!("{} points at a dangling parent", node.describe()));
                    };
                    if !parent.children.contains(&h) {
                        return Some(format!(
                            "{} is not listed in its parent's children",
                            node.describe()
                        ));
                    }
                    if parent.kind.child_kind() != Some(node.kind) {
                        return Some(format!("a {} contains a {}", parent.kind, node.kind));
                    }
                    if node.state > parent.state {
                        return Some(format!(
                            "{} is {} but its parent is only {}",
                            node.describe(),
                            node.state,
                            parent.state
                        ));
                    }
                }
                None if h != self.root => {
                    return Some(format!(
                        "{} is parentless but is not the root",
                        node.describe()
                    ))
                }
                None => {}
            }
            // Adjacency is symmetric and same-kind.
            for peer in &node.neighbors {
                let Some(other) = self.get(*peer) else {
                    return Some(format!(
                        "{} points at a dangling neighbour",
                        node.describe()
                    ));
                };
                if !other.neighbors.contains(&h) {
                    return Some(format!(
                        "{} has a one-sided adjacency link",
                        node.describe()
                    ));
                }
                if other.kind != node.kind {
                    return Some(format!(
                        "{} is adjacent to a {}",
                        node.describe(),
                        other.kind
                    ));
                }
            }
            // Anything past Projected must have claimed space.
            if node.state != NodeState::Projected && node.envelope.is_none() {
                return Some(format!(
                    "{} is {} without an envelope",
                    node.describe(),
                    node.state
                ));
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------------------------

impl Serialize for NodeKind {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.depth());
    }
}

impl Deserialize for NodeKind {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        NodeKind::from_tag(r.u8()?).ok_or(SerError::InvalidValue("unknown NodeKind tag"))
    }
}

impl Serialize for NodeState {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.tag());
    }
}

impl Deserialize for NodeState {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        NodeState::from_tag(r.u8()?).ok_or(SerError::InvalidValue("unknown NodeState tag"))
    }
}

impl Serialize for Node {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.header);
        w.write(&self.kind);
        w.write(&self.state);
        w.write(&self.parent);
        w.write(&self.children);
        w.write(&self.neighbors);
        w.write(&self.envelope);
    }
}

impl Deserialize for Node {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Node {
            header: r.read()?,
            kind: r.read()?,
            state: r.read()?,
            parent: r.read()?,
            children: r.read()?,
            neighbors: r.read()?,
            envelope: r.read()?,
        })
    }
}

impl Serialize for NodeGraph {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.nodes);
        w.write(&self.root);
        w.write(&self.ids);
        w.f64(self.scale);
        w.u64(self.seed);
    }
}

impl Deserialize for NodeGraph {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let nodes: Arena<Node> = r.read()?;
        let root: Handle<Node> = r.read()?;
        let ids = r.read()?;
        let scale = r.f64()?;
        let seed = r.u64()?;
        if nodes.get(root).is_none() {
            return Err(SerError::InvalidValue("node graph root handle is not live"));
        }
        let graph = NodeGraph {
            nodes,
            root,
            ids,
            scale,
            seed,
        };
        // A corrupt or hand-edited bundle must not load as a subtly-broken graph: dangling parents,
        // one-sided adjacency, or a child ahead of its parent would all produce wrong worlds later,
        // far from the cause. Validate once, here, where the error is still explainable.
        if graph.check_invariants().is_some() {
            return Err(SerError::InvalidValue(
                "node graph violates its structural invariants",
            ));
        }
        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cv_determinism::Vec3;

    fn unit_box() -> Aabb {
        Aabb::new(Vec3::ZERO, Vec3::ONE)
    }

    /// World → Reach → Area → Space → Spatial, all projected.
    fn chain() -> (NodeGraph, Vec<Handle<Node>>) {
        let mut g = NodeGraph::new(1.0, 42);
        let mut hs = vec![g.root()];
        for name in ["reach", "area", "space", "spatial"] {
            let parent = *hs.last().unwrap();
            hs.push(g.add_child(parent, name).unwrap());
        }
        (g, hs)
    }

    #[test]
    fn hierarchy_is_derived_not_declared() {
        let (g, hs) = chain();
        let kinds: Vec<NodeKind> = hs.iter().map(|h| g.node(*h).unwrap().kind()).collect();
        assert_eq!(kinds, NodeKind::ALL.to_vec());
        assert!(g.check_invariants().is_none());
    }

    #[test]
    fn a_spatial_cannot_contain_anything() {
        let (mut g, hs) = chain();
        let spatial = hs[4];
        assert_eq!(
            g.add_child(spatial, "nope"),
            Err(NodeError::NotAContainer {
                kind: NodeKind::Spatial
            })
        );
    }

    #[test]
    fn a_child_may_lag_behind_its_parent_but_never_lead() {
        let (mut g, hs) = chain();
        for h in &hs {
            g.set_envelope(*h, unit_box()).unwrap();
        }
        // A Reach cannot reserve before the World does.
        assert_eq!(
            g.advance(hs[1], NodeState::Reserved),
            Err(NodeError::AheadOfParent {
                parent: NodeState::Projected,
                requested: NodeState::Reserved
            })
        );
        // Top-down works.
        g.advance(hs[0], NodeState::Reserved).unwrap();
        g.advance(hs[1], NodeState::Reserved).unwrap();
        // And lagging is fine: the World may realize while its Reach stays merely reserved.
        g.advance(hs[0], NodeState::Realized).unwrap();
        assert_eq!(g.node(hs[1]).unwrap().state(), NodeState::Reserved);
        assert_eq!(g.node(hs[2]).unwrap().state(), NodeState::Projected);
        assert!(g.check_invariants().is_none());
    }

    #[test]
    fn the_lifecycle_is_one_way() {
        let (mut g, hs) = chain();
        g.set_envelope(hs[0], unit_box()).unwrap();
        g.advance(hs[0], NodeState::Realized).unwrap();
        assert_eq!(
            g.advance(hs[0], NodeState::Projected),
            Err(NodeError::Regression {
                from: NodeState::Realized,
                to: NodeState::Projected
            })
        );
        // Re-advancing to the same state is a no-op, not an error.
        assert!(g.advance(hs[0], NodeState::Realized).is_ok());
    }

    #[test]
    fn reserving_requires_an_envelope() {
        let (mut g, hs) = chain();
        assert_eq!(
            g.advance(hs[0], NodeState::Reserved),
            Err(NodeError::MissingEnvelope {
                kind: NodeKind::World
            })
        );
        g.set_envelope(hs[0], unit_box()).unwrap();
        assert!(g.advance(hs[0], NodeState::Reserved).is_ok());
    }

    #[test]
    fn realized_nodes_reject_every_mutation() {
        let (mut g, hs) = chain();
        let world = hs[0];
        g.set_envelope(world, unit_box()).unwrap();
        g.advance(world, NodeState::Realized).unwrap();

        let frozen = NodeError::Immutable {
            kind: NodeKind::World,
            state: NodeState::Realized,
        };
        assert_eq!(g.set_name(world, "x"), Err(frozen.clone()));
        assert_eq!(g.set_envelope(world, unit_box()), Err(frozen));
        // There is no API returning `&mut Node`, so no other mutation path exists.
    }

    #[test]
    fn a_realized_container_can_still_gain_lazy_children() {
        // The lazy-generation case: the World is built, and a new Reach streams in afterwards. If
        // this were forbidden, the World could never realize while regions remained undiscovered.
        let (mut g, hs) = chain();
        let world = hs[0];
        g.set_envelope(world, unit_box()).unwrap();
        g.advance(world, NodeState::Realized).unwrap();

        let late = g.add_child(world, "streamed_in").unwrap();
        assert_eq!(g.node(late).unwrap().state(), NodeState::Projected);
        assert_eq!(g.node(late).unwrap().kind(), NodeKind::Reach);
        // It can be realized in turn, because its parent already is.
        g.set_envelope(late, unit_box()).unwrap();
        g.advance(late, NodeState::Realized).unwrap();
        assert!(g.check_invariants().is_none());

        // Adjacency, by contrast, stays frozen once realized.
        let other = g.add_child(world, "other").unwrap();
        g.set_envelope(other, unit_box()).unwrap();
        g.advance(other, NodeState::Realized).unwrap();
        assert!(matches!(
            g.connect(late, other),
            Err(NodeError::Immutable { .. })
        ));
    }

    #[test]
    fn only_projected_nodes_can_be_removed() {
        let (mut g, hs) = chain();
        // The root is never removable.
        assert_eq!(g.remove(hs[0]), Err(NodeError::RootNode));

        // Committing a descendant pins the whole chain above it.
        for h in &hs[..3] {
            g.set_envelope(*h, unit_box()).unwrap();
            g.advance(*h, NodeState::Reserved).unwrap();
        }
        assert_eq!(
            g.remove(hs[1]),
            Err(NodeError::NotProjected {
                state: NodeState::Reserved
            })
        );
        // A wholly-projected subtree goes cleanly.
        let extra = g.add_child(hs[2], "throwaway").unwrap();
        let leaf = g.add_child(extra, "leaf").unwrap();
        assert!(g.remove(extra).is_ok());
        assert!(g.get(extra).is_none());
        assert!(g.get(leaf).is_none(), "the subtree must go with it");
        assert!(g.check_invariants().is_none());
    }

    #[test]
    fn a_committed_descendant_blocks_removal() {
        let (mut g, hs) = chain();
        for h in &hs[..4] {
            g.set_envelope(*h, unit_box()).unwrap();
            g.advance(*h, NodeState::Reserved).unwrap();
        }
        // hs[3] (Space) is Reserved; its projected parent chain is pinned by it.
        let branch = g.add_child(hs[2], "branch").unwrap();
        let deep = g.add_child(branch, "deep").unwrap();
        g.set_envelope(branch, unit_box()).unwrap();
        g.set_envelope(deep, unit_box()).unwrap();
        g.advance(branch, NodeState::Reserved).unwrap();
        g.advance(deep, NodeState::Reserved).unwrap();
        // `branch` itself is Reserved, so removal fails on the node, not the descendant.
        assert!(matches!(
            g.remove(branch),
            Err(NodeError::NotProjected { .. })
        ));
    }

    #[test]
    fn adjacency_is_symmetric_same_kind_and_idempotent() {
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "r").unwrap();
        let area = g.add_child(reach, "a").unwrap();
        let s1 = g.add_child(area, "s1").unwrap();
        let s2 = g.add_child(area, "s2").unwrap();

        g.connect(s1, s2).unwrap();
        g.connect(s1, s2).unwrap(); // idempotent
        assert_eq!(g.node(s1).unwrap().neighbors(), &[s2]);
        assert_eq!(g.node(s2).unwrap().neighbors(), &[s1]);

        assert_eq!(g.connect(s1, s1), Err(NodeError::SelfAdjacency));
        assert_eq!(
            g.connect(s1, area),
            Err(NodeError::KindMismatch {
                a: NodeKind::Space,
                b: NodeKind::Area
            })
        );
        assert!(g.check_invariants().is_none());

        g.disconnect(s1, s2).unwrap();
        assert!(g.node(s1).unwrap().neighbors().is_empty());
        assert!(g.node(s2).unwrap().neighbors().is_empty());
        g.disconnect(s1, s2).unwrap(); // idempotent
    }

    #[test]
    fn removing_a_node_leaves_no_dangling_adjacency() {
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "r").unwrap();
        let area = g.add_child(reach, "a").unwrap();
        let s1 = g.add_child(area, "s1").unwrap();
        let s2 = g.add_child(area, "s2").unwrap();
        let s3 = g.add_child(area, "s3").unwrap();
        g.connect(s1, s2).unwrap();
        g.connect(s2, s3).unwrap();

        g.remove(s2).unwrap();
        assert!(g.node(s1).unwrap().neighbors().is_empty());
        assert!(g.node(s3).unwrap().neighbors().is_empty());
        assert!(
            g.check_invariants().is_none(),
            "no one-sided links may survive"
        );
    }

    #[test]
    fn advance_with_ancestors_establishes_the_chain_top_down() {
        let (mut g, hs) = chain();
        for h in &hs {
            g.set_envelope(*h, unit_box()).unwrap();
        }
        g.advance_with_ancestors(hs[4], NodeState::Realized)
            .unwrap();
        for h in &hs {
            assert_eq!(g.node(*h).unwrap().state(), NodeState::Realized);
        }
        assert!(g.check_invariants().is_none());
    }

    #[test]
    fn queries_answer_scope_questions() {
        let (g, hs) = chain();
        let spatial = hs[4];
        assert_eq!(g.scope_of(spatial, NodeKind::Space), Some(hs[3]));
        assert_eq!(g.scope_of(spatial, NodeKind::World), Some(hs[0]));
        assert_eq!(
            g.scope_of(spatial, NodeKind::Spatial),
            Some(spatial),
            "a node is its own scope"
        );
        assert_eq!(g.depth_of(spatial), 4);
        assert_eq!(g.ancestors_of(spatial), vec![hs[3], hs[2], hs[1], hs[0]]);
        assert_eq!(g.descendants_of(hs[0]), vec![hs[1], hs[2], hs[3], hs[4]]);
        assert_eq!(g.walk(), hs);
        assert_eq!(g.of_kind(NodeKind::Space).count(), 1);
        assert_eq!(g.find(|n| n.state() == NodeState::Projected).count(), 5);
    }

    #[test]
    fn traversal_is_depth_first_in_child_order() {
        let mut g = NodeGraph::new(1.0, 1);
        let r1 = g.add_child(g.root(), "r1").unwrap();
        let r2 = g.add_child(g.root(), "r2").unwrap();
        let a1 = g.add_child(r1, "a1").unwrap();
        let a2 = g.add_child(r1, "a2").unwrap();
        let a3 = g.add_child(r2, "a3").unwrap();
        // Depth-first, children in insertion order — not arena order, not sorted.
        assert_eq!(g.descendants_of(g.root()), vec![r1, a1, a2, r2, a3]);
    }

    #[test]
    fn building_the_same_world_twice_is_identical() {
        fn build() -> NodeGraph {
            let mut g = NodeGraph::new(2.5, 0xABCD);
            let reach = g.add_child(g.root(), "reach").unwrap();
            let area = g.add_child(reach, "area").unwrap();
            let spaces: Vec<_> = (0..4)
                .map(|i| g.add_child(area, format!("space_{i}")).unwrap())
                .collect();
            for w in spaces.windows(2) {
                g.connect(w[0], w[1]).unwrap();
            }
            let doomed = g.add_child(area, "doomed").unwrap();
            g.remove(doomed).unwrap();
            for h in [g.root(), reach, area] {
                g.set_envelope(h, unit_box()).unwrap();
                g.advance(h, NodeState::Reserved).unwrap();
            }
            g
        }
        assert_eq!(build(), build());
        assert!(build().check_invariants().is_none());
    }

    #[test]
    fn stale_handles_are_rejected() {
        let mut g = NodeGraph::new(1.0, 1);
        let reach = g.add_child(g.root(), "r").unwrap();
        g.remove(reach).unwrap();
        assert_eq!(g.node(reach), Err(NodeError::StaleHandle));
        assert_eq!(g.set_name(reach, "x"), Err(NodeError::StaleHandle));
        assert_eq!(
            g.advance(reach, NodeState::Reserved),
            Err(NodeError::StaleHandle)
        );
        assert!(g.get(reach).is_none());
    }
}
