//! The **`WorldDescriptor`** — what generation actually hands back, and the only thing a host has to
//! understand.
//!
//! # It is structural data, not content
//!
//! The descriptor says *"a `door_heavy` goes here, at this transform, because the solver needed a gate
//! on this edge"*. It does **not** contain a door. No vertices, no textures, no audio — the host owns
//! all of that and loads it through whatever pipeline it already has. What the descriptor supplies is
//! the part only the generator knows: **what goes where, and why**.
//!
//! That boundary is deliberate and load-bearing:
//!
//! * A studio keeps its asset pipeline, however proprietary. CycleVania never needs to read its formats.
//! * The output stays small and diffable — a whole world is kilobytes, so a reproduction bundle can
//!   travel in a bug report.
//! * Swapping a mesh is a host-side change; it does not invalidate a generated world.
//!
//! # Host-shaped, not engine-shaped
//!
//! Internally the scope graph is an arena of generational handles ([`crate::node`]). A host should not
//! have to learn that, so the descriptor **flattens** the tree into a `Vec` addressed by
//! [`ScopeRef`] — a plain index. Records are emitted in deterministic depth-first order, so a host can
//! walk them top-down in a single pass without building its own index first.
//!
//! # Lazily populated
//!
//! A descriptor describes the world *as far as it has been generated*. Scopes carry their
//! [`NodeState`], so a host can tell a built room from a forecast and stream the rest later.

use crate::content::ContentRegistry;
use crate::fingerprint::Fingerprint;
use crate::node::{Node, NodeGraph, NodeKind, NodeState};
use crate::object::{Object, ObjectId};
use crate::serialize::{Deserialize, Reader, SerError, SerResult, Serialize, Writer};
use crate::Handle;
use cv_determinism::{Aabb, Mat4, Transform};
use std::collections::BTreeMap;
use std::fmt;

/// The descriptor schema version, independent of the binary format version.
///
/// The wire format and the *shape of the data* change for different reasons, so they version
/// separately: a host can support schema 1 while the encoding gains a new primitive.
pub const SCHEMA_VERSION: u32 = 1;

/// An index into [`WorldDescriptor::scopes`].
///
/// Deliberately a bare index rather than a [`Handle`]: a host consuming JSON or a foreign binding
/// should not need the arena's generational-handle concept to walk a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeRef(pub u32);

impl ScopeRef {
    /// As a `usize` for indexing.
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Display for ScopeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scope[{}]", self.0)
    }
}

// ---------------------------------------------------------------------------------------------
// Placement
// ---------------------------------------------------------------------------------------------

/// Where and how something sits in the world.
///
/// # Why this is an enum
///
/// Two things are true at once, and one representation cannot serve both well:
///
/// * **Almost every placement is translate-rotate-scale**, and every engine on earth wants exactly
///   those three fields. Forcing hosts to decompose a matrix for the common case would be gratuitous.
/// * **TRS cannot express shear**, which arises whenever a rotated thing sits inside a non-uniformly
///   scaled parent (see [`Mat4`]). Silently dropping it would corrupt those placements.
///
/// So the common case is [`Placement::Trs`] and the rare one is [`Placement::Affine`]. A host that
/// only handles TRS can check [`Placement::as_transform`] and fall back to
/// [`Placement::to_matrix`] — and most will never see an `Affine` at all.
///
/// **Mirroring is *not* the exception here.** A reflection decomposes into TRS with a negative scale,
/// so it stays in the common case — but it flips triangle winding, which a host must handle. Ask
/// [`Placement::is_mirroring`] rather than inspecting the scale by hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// Translation, rotation, scale. Scale may be negative on an axis, which means mirrored.
    Trs(Transform),
    /// A general affine matrix, used only when the placement contains shear.
    Affine(Mat4),
}

impl Placement {
    /// The identity placement.
    pub const IDENTITY: Placement = Placement::Trs(Transform::IDENTITY);

    /// Build from a matrix, decomposing to TRS when possible so hosts get the easy form.
    pub fn from_matrix(m: Mat4) -> Placement {
        match m.to_transform() {
            Some(t) => Placement::Trs(t),
            None => Placement::Affine(m), // sheared
        }
    }

    /// As a matrix — always available.
    pub fn to_matrix(self) -> Mat4 {
        match self {
            Placement::Trs(t) => Mat4::from(t),
            Placement::Affine(m) => m,
        }
    }

    /// As TRS, or `None` when the placement is sheared.
    pub fn as_transform(self) -> Option<Transform> {
        match self {
            Placement::Trs(t) => Some(t),
            Placement::Affine(m) => m.to_transform(),
        }
    }

    /// Does this placement flip handedness?
    ///
    /// Mirrored instances need their triangle winding reversed by the host, or they render inside-out.
    pub fn is_mirroring(self) -> bool {
        match self {
            // Cheap path: a TRS mirrors iff an odd number of scale axes are negative.
            Placement::Trs(t) => (t.scale.x * t.scale.y * t.scale.z) < 0.0,
            Placement::Affine(m) => m.is_mirroring(),
        }
    }

    /// Apply to a point.
    pub fn transform_point(self, p: cv_determinism::Vec3) -> cv_determinism::Vec3 {
        match self {
            Placement::Trs(t) => t.transform_point(p),
            Placement::Affine(m) => m.transform_point(p),
        }
    }
}

impl From<Transform> for Placement {
    fn from(t: Transform) -> Placement {
        Placement::Trs(t)
    }
}

// ---------------------------------------------------------------------------------------------
// Rationale
// ---------------------------------------------------------------------------------------------

/// Why the generator did something.
///
/// Carried in the output rather than only in a debug log, because "why is this door here?" is the
/// question a dev asks constantly while tuning, and answering it after the fact is impossible — the
/// reasoning exists only at the moment of the decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlacementReason {
    /// L1's schedule called for it.
    Scheduled,
    /// L2 required it for solvability — a key, a gate, a recovery affordance.
    SolverRequired,
    /// Structural: a connector or transition between scopes.
    Connector,
    /// Legibility: an indicator making a remote cause/effect readable.
    Indicator,
    /// Aesthetic fill with no logical role.
    Dressing,
    /// The host asked for it explicitly.
    HostRequested,
}

impl PlacementReason {
    /// All reasons, in tag order.
    pub const ALL: [PlacementReason; 6] = [
        PlacementReason::Scheduled,
        PlacementReason::SolverRequired,
        PlacementReason::Connector,
        PlacementReason::Indicator,
        PlacementReason::Dressing,
        PlacementReason::HostRequested,
    ];

    /// Is this placement load-bearing for solvability? Removing one that is will break the world.
    pub fn is_required(self) -> bool {
        matches!(
            self,
            PlacementReason::SolverRequired | PlacementReason::Connector
        )
    }

    fn tag(self) -> u8 {
        PlacementReason::ALL
            .iter()
            .position(|r| *r == self)
            .unwrap_or(0) as u8
    }

    fn from_tag(tag: u8) -> Option<Self> {
        PlacementReason::ALL.get(tag as usize).copied()
    }

    /// The name shown in traces and the editor.
    pub fn as_str(self) -> &'static str {
        match self {
            PlacementReason::Scheduled => "scheduled",
            PlacementReason::SolverRequired => "solver-required",
            PlacementReason::Connector => "connector",
            PlacementReason::Indicator => "indicator",
            PlacementReason::Dressing => "dressing",
            PlacementReason::HostRequested => "host-requested",
        }
    }
}

impl fmt::Display for PlacementReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A reason plus a human-readable detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rationale {
    /// The machine-readable category — safe to branch on.
    pub reason: PlacementReason,
    /// A short human explanation for the trace and the inspector. May be empty.
    pub detail: String,
}

impl Rationale {
    /// A rationale with no detail.
    pub fn new(reason: PlacementReason) -> Self {
        Rationale {
            reason,
            detail: String::new(),
        }
    }

    /// A rationale with an explanation.
    pub fn detailed(reason: PlacementReason, detail: impl Into<String>) -> Self {
        Rationale {
            reason,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Rationale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.detail.is_empty() {
            write!(f, "{}", self.reason)
        } else {
            write!(f, "{}: {}", self.reason, self.detail)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------------------------

/// One scope in the flattened tree.
#[derive(Clone, Debug, PartialEq)]
pub struct ScopeRecord {
    /// Stable identity, matching the internal node.
    pub id: ObjectId,
    /// Human-facing name.
    pub name: String,
    /// Where it sits in the hierarchy.
    pub kind: NodeKind,
    /// How far along generation it is — `Projected` means "forecast, not built".
    pub state: NodeState,
    /// The containing scope; `None` only for the World root, which is always index 0.
    pub parent: Option<ScopeRef>,
    /// Spatial bounds, once claimed.
    pub envelope: Option<Aabb>,
    /// Spatially adjacent peers.
    pub neighbors: Vec<ScopeRef>,
    /// The spine slot this scope was allocated to, if any.
    ///
    /// **This is how a host finds the room a spine guaranteed.** A roguelite must know *which* Space
    /// is the rest room in order to place its save interaction; a Zelda-like must know which is the
    /// boss arena. Being told beats re-deriving structure the generator already knew.
    pub spine_slot: Option<SpineSlotTag>,
}

/// Identifies which spine slot a scope was allocated to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpineSlotTag {
    /// The template that placed it.
    pub template: ObjectId,
    /// The slot's declared name — `"capstone"`, `"terminal"`.
    pub slot: String,
}

/// A placed piece of content.
#[derive(Clone, Debug, PartialEq)]
pub struct InstanceRecord {
    /// This instance's identity — distinct from the content it instantiates.
    pub id: ObjectId,
    /// Which registered content this is; look it up in the [`ContentRegistry`].
    pub content: ObjectId,
    /// The scope it belongs to.
    pub scope: ScopeRef,
    /// Where it sits, relative to its scope.
    pub placement: Placement,
    /// Why the generator put it here.
    pub rationale: Rationale,
}

/// A named attachment point on a mesh — where a door hinges, where a corridor connects.
#[derive(Clone, Debug, PartialEq)]
pub struct Socket {
    /// The socket's name, as authored.
    pub name: String,
    /// Its transform, relative to the mesh.
    pub transform: Transform,
}

/// **Metadata about a placed mesh — never the mesh itself.**
///
/// Everything here is *about* geometry the host already owns: which asset, where it goes, what shape
/// it blocks, where things attach. The vertex data stays in the host's pipeline. That is why a record
/// is a fixed handful of fields regardless of whether the mesh is a crate or a cathedral.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshRecord {
    /// This placement's identity.
    pub id: ObjectId,
    /// The registered `StaticMesh` content id — a *reference*, which the host resolves to its own asset.
    pub mesh: ObjectId,
    /// The scope it belongs to.
    pub scope: ScopeRef,
    /// Where it sits.
    pub placement: Placement,
    /// Coarse collision volumes. Metadata the generator reasoned with, not a collision *mesh*.
    pub collision: Vec<Aabb>,
    /// Named attachment points.
    pub sockets: Vec<Socket>,
    /// `SurfaceProperty` content ids applied to this mesh.
    pub tags: Vec<ObjectId>,
    /// Why it was placed here.
    pub rationale: Rationale,
}

impl MeshRecord {
    /// Does this placement need its triangle winding reversed by the host?
    pub fn needs_winding_flip(&self) -> bool {
        self.placement.is_mirroring()
    }

    /// Find a socket by name.
    pub fn socket(&self, name: &str) -> Option<&Socket> {
        self.sockets.iter().find(|s| s.name == name)
    }
}

// ---------------------------------------------------------------------------------------------
// WorldDescriptor
// ---------------------------------------------------------------------------------------------

/// The complete output of a generation run.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldDescriptor {
    /// The schema this was written against.
    pub schema_version: u32,
    /// The recipe that produced it.
    pub fingerprint: Fingerprint,
    /// The seed that selected it.
    pub seed: u64,
    /// World units per metre.
    pub scale: f64,
    /// The scope tree, flattened depth-first. Index 0 is always the World root.
    pub scopes: Vec<ScopeRecord>,
    /// Placed content.
    pub instances: Vec<InstanceRecord>,
    /// Placed mesh metadata.
    pub meshes: Vec<MeshRecord>,
}

impl WorldDescriptor {
    /// The World root record.
    pub fn root(&self) -> Option<&ScopeRecord> {
        self.scopes.first()
    }

    /// Look up a scope.
    pub fn scope(&self, r: ScopeRef) -> Option<&ScopeRecord> {
        self.scopes.get(r.index())
    }

    /// The direct children of a scope, in order.
    ///
    /// Derived rather than stored: a parent link plus depth-first ordering already determines it, and
    /// storing both invites the two disagreeing.
    pub fn children_of(&self, r: ScopeRef) -> impl Iterator<Item = (ScopeRef, &ScopeRecord)> + '_ {
        self.scopes
            .iter()
            .enumerate()
            .filter(move |(_, s)| s.parent == Some(r))
            .map(|(i, s)| (ScopeRef(i as u32), s))
    }

    /// Every scope of a kind.
    pub fn scopes_of_kind(
        &self,
        kind: NodeKind,
    ) -> impl Iterator<Item = (ScopeRef, &ScopeRecord)> + '_ {
        self.scopes
            .iter()
            .enumerate()
            .filter(move |(_, s)| s.kind == kind)
            .map(|(i, s)| (ScopeRef(i as u32), s))
    }

    /// Everything placed in a scope.
    pub fn instances_in(&self, r: ScopeRef) -> impl Iterator<Item = &InstanceRecord> + '_ {
        self.instances.iter().filter(move |i| i.scope == r)
    }

    /// Every mesh placed in a scope.
    pub fn meshes_in(&self, r: ScopeRef) -> impl Iterator<Item = &MeshRecord> + '_ {
        self.meshes.iter().filter(move |m| m.scope == r)
    }

    /// The scope allocated to a named spine slot, if a spine ran.
    ///
    /// This is the host's entry point to a guarantee: `spine_slot(template, "terminal")` answers
    /// "which room is the treasury?" without re-deriving structure the generator already knew.
    pub fn spine_slot(&self, template: ObjectId, slot: &str) -> Option<(ScopeRef, &ScopeRecord)> {
        self.spine_slots(template, slot).next()
    }

    /// Every scope allocated to a named spine slot — one per covered instance.
    pub fn spine_slots(
        &self,
        template: ObjectId,
        slot: &str,
    ) -> impl Iterator<Item = (ScopeRef, &ScopeRecord)> + '_ {
        let slot = slot.to_string();
        self.scopes
            .iter()
            .enumerate()
            .filter(move |(_, s)| {
                s.spine_slot
                    .as_ref()
                    .is_some_and(|t| t.template == template && t.slot == slot)
            })
            .map(|(i, s)| (ScopeRef(i as u32), s))
    }

    /// Scopes a host can build right now — everything not still a forecast.
    pub fn realized_scopes(&self) -> impl Iterator<Item = (ScopeRef, &ScopeRecord)> + '_ {
        self.scopes
            .iter()
            .enumerate()
            .filter(|(_, s)| s.state == NodeState::Realized)
            .map(|(i, s)| (ScopeRef(i as u32), s))
    }

    /// Check the descriptor is internally consistent — indices in range, root well-formed, parents
    /// preceding children. Returns the first problem found.
    ///
    /// A host is entitled to assume all of this; this is what lets it walk the arrays in one pass.
    pub fn check(&self) -> Option<String> {
        if self.schema_version != SCHEMA_VERSION {
            return Some(format!(
                "schema version {} is not {SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        match self.scopes.first() {
            None => return Some("descriptor has no scopes; the World root is mandatory".into()),
            Some(root) if root.parent.is_some() => {
                return Some("scope[0] must be the parentless World root".into())
            }
            Some(root) if root.kind != NodeKind::World => {
                return Some(format!("scope[0] is a {} rather than the World", root.kind))
            }
            _ => {}
        }
        let n = self.scopes.len() as u32;
        for (i, s) in self.scopes.iter().enumerate() {
            if let Some(p) = s.parent {
                if p.0 >= n {
                    return Some(format!("scope[{i}] parent {p} is out of range"));
                }
                if p.0 as usize >= i {
                    // Depth-first order guarantees this; a host relies on it to build top-down.
                    return Some(format!("scope[{i}] parent {p} does not precede it"));
                }
            } else if i != 0 {
                return Some(format!("scope[{i}] is parentless but is not the root"));
            }
            for peer in &s.neighbors {
                if peer.0 >= n {
                    return Some(format!("scope[{i}] neighbour {peer} is out of range"));
                }
            }
        }
        for (i, inst) in self.instances.iter().enumerate() {
            if inst.scope.0 >= n {
                return Some(format!(
                    "instance[{i}] scope {} is out of range",
                    inst.scope
                ));
            }
        }
        for (i, m) in self.meshes.iter().enumerate() {
            if m.scope.0 >= n {
                return Some(format!("mesh[{i}] scope {} is out of range", m.scope));
            }
        }
        None
    }

    /// Every content id the descriptor references but the registry does not define.
    ///
    /// A host calls this once after loading: an empty result means every reference resolves, so it can
    /// look content up without handling a miss on every record.
    pub fn unresolved_content(&self, registry: &ContentRegistry) -> Vec<ObjectId> {
        let mut missing: Vec<ObjectId> = Vec::new();
        let note = |id: ObjectId, missing: &mut Vec<ObjectId>| {
            if !registry.contains(id) && !missing.contains(&id) {
                missing.push(id);
            }
        };
        for i in &self.instances {
            note(i.content, &mut missing);
        }
        for m in &self.meshes {
            note(m.mesh, &mut missing);
            for t in &m.tags {
                note(*t, &mut missing);
            }
        }
        missing
    }
}

/// Builds a [`WorldDescriptor`] from the internal graph.
///
/// Flattening happens here so the rest of the engine never has to think in host terms, and the host
/// never has to think in engine terms.
pub struct DescriptorBuilder {
    descriptor: WorldDescriptor,
    /// Internal handle → flattened index.
    refs: BTreeMap<Handle<Node>, ScopeRef>,
}

impl DescriptorBuilder {
    /// Flatten a scope graph. Scopes come out in deterministic depth-first order, root first.
    pub fn new(graph: &NodeGraph, fingerprint: Fingerprint, seed: u64) -> Self {
        let order = graph.walk();
        let mut refs = BTreeMap::new();
        for (i, h) in order.iter().enumerate() {
            refs.insert(*h, ScopeRef(i as u32));
        }

        let scopes = order
            .iter()
            .map(|h| {
                let node = graph.node(*h).expect("walk yields live handles");
                ScopeRecord {
                    id: node.id(),
                    name: node.name().to_string(),
                    kind: node.kind(),
                    state: node.state(),
                    parent: node.parent().map(|p| refs[&p]),
                    envelope: node.envelope(),
                    // Neighbours are sorted by index so the record is canonical regardless of the
                    // order links happened to be created in.
                    neighbors: {
                        let mut n: Vec<ScopeRef> = node
                            .neighbors()
                            .iter()
                            .filter_map(|p| refs.get(p).copied())
                            .collect();
                        n.sort();
                        n
                    },
                    // Filled in by the spine pass if one ran; `None` is the normal case.
                    spine_slot: None,
                }
            })
            .collect();

        DescriptorBuilder {
            descriptor: WorldDescriptor {
                schema_version: SCHEMA_VERSION,
                fingerprint,
                seed,
                scale: graph.scale(),
                scopes,
                instances: Vec::new(),
                meshes: Vec::new(),
            },
            refs,
        }
    }

    /// The flattened index for an internal handle.
    pub fn scope_ref(&self, h: Handle<Node>) -> Option<ScopeRef> {
        self.refs.get(&h).copied()
    }

    /// Record a placed piece of content.
    pub fn place(&mut self, record: InstanceRecord) -> &mut Self {
        self.descriptor.instances.push(record);
        self
    }

    /// Record placed mesh metadata.
    pub fn place_mesh(&mut self, record: MeshRecord) -> &mut Self {
        self.descriptor.meshes.push(record);
        self
    }

    /// Tag a scope as the one a spine slot was allocated to.
    ///
    /// Returns `false` if the handle is not in this descriptor. Hosts read the tag back through
    /// [`WorldDescriptor::spine_slot`]: a guaranteed room is only useful if the host can *find* it.
    pub fn tag_spine_slot(&mut self, h: Handle<Node>, tag: SpineSlotTag) -> bool {
        let Some(r) = self.scope_ref(h) else {
            return false;
        };
        let Some(record) = self.descriptor.scopes.get_mut(r.0 as usize) else {
            return false;
        };
        record.spine_slot = Some(tag);
        true
    }

    /// Finish, yielding the descriptor.
    pub fn finish(self) -> WorldDescriptor {
        self.descriptor
    }
}

// ---------------------------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------------------------

impl Serialize for ScopeRef {
    fn serialize(&self, w: &mut Writer) {
        w.u32(self.0);
    }
}

impl Deserialize for ScopeRef {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(ScopeRef(r.u32()?))
    }
}

impl Serialize for Placement {
    fn serialize(&self, w: &mut Writer) {
        match self {
            Placement::Trs(t) => {
                w.u8(0);
                w.write(t);
            }
            Placement::Affine(m) => {
                w.u8(1);
                w.write(m);
            }
        }
    }
}

impl Deserialize for Placement {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        match r.u8()? {
            0 => Ok(Placement::Trs(r.read()?)),
            1 => Ok(Placement::Affine(r.read()?)),
            _ => Err(SerError::InvalidValue("unknown Placement tag")),
        }
    }
}

impl Serialize for PlacementReason {
    fn serialize(&self, w: &mut Writer) {
        w.u8(self.tag());
    }
}

impl Deserialize for PlacementReason {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        PlacementReason::from_tag(r.u8()?)
            .ok_or(SerError::InvalidValue("unknown PlacementReason tag"))
    }
}

impl Serialize for Rationale {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.reason);
        w.str(&self.detail);
    }
}

impl Deserialize for Rationale {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Rationale {
            reason: r.read()?,
            detail: r.str()?,
        })
    }
}

impl Serialize for ScopeRecord {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.id);
        w.str(&self.name);
        w.write(&self.kind);
        w.write(&self.state);
        w.write(&self.parent);
        w.write(&self.envelope);
        w.write(&self.neighbors);
        w.write(&self.spine_slot);
    }
}

impl Deserialize for ScopeRecord {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(ScopeRecord {
            id: r.read()?,
            name: r.str()?,
            kind: r.read()?,
            state: r.read()?,
            parent: r.read()?,
            envelope: r.read()?,
            neighbors: r.read()?,
            spine_slot: r.read()?,
        })
    }
}

impl Serialize for SpineSlotTag {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.template);
        w.str(&self.slot);
    }
}

impl Deserialize for SpineSlotTag {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(SpineSlotTag {
            template: r.read()?,
            slot: r.str()?,
        })
    }
}

impl Serialize for InstanceRecord {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.id);
        w.write(&self.content);
        w.write(&self.scope);
        w.write(&self.placement);
        w.write(&self.rationale);
    }
}

impl Deserialize for InstanceRecord {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(InstanceRecord {
            id: r.read()?,
            content: r.read()?,
            scope: r.read()?,
            placement: r.read()?,
            rationale: r.read()?,
        })
    }
}

impl Serialize for Socket {
    fn serialize(&self, w: &mut Writer) {
        w.str(&self.name);
        w.write(&self.transform);
    }
}

impl Deserialize for Socket {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Socket {
            name: r.str()?,
            transform: r.read()?,
        })
    }
}

impl Serialize for MeshRecord {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.id);
        w.write(&self.mesh);
        w.write(&self.scope);
        w.write(&self.placement);
        w.write(&self.collision);
        w.write(&self.sockets);
        w.write(&self.tags);
        w.write(&self.rationale);
    }
}

impl Deserialize for MeshRecord {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(MeshRecord {
            id: r.read()?,
            mesh: r.read()?,
            scope: r.read()?,
            placement: r.read()?,
            collision: r.read()?,
            sockets: r.read()?,
            tags: r.read()?,
            rationale: r.read()?,
        })
    }
}

impl Serialize for WorldDescriptor {
    fn serialize(&self, w: &mut Writer) {
        w.u32(self.schema_version);
        w.write(&self.fingerprint);
        w.u64(self.seed);
        w.f64(self.scale);
        w.write(&self.scopes);
        w.write(&self.instances);
        w.write(&self.meshes);
    }
}

impl Deserialize for WorldDescriptor {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let d = WorldDescriptor {
            schema_version: r.u32()?,
            fingerprint: r.read()?,
            seed: r.u64()?,
            scale: r.f64()?,
            scopes: r.read()?,
            instances: r.read()?,
            meshes: r.read()?,
        };
        // Validate at load, where the error is still explainable — a host walking a malformed
        // descriptor would fail somewhere far from the cause.
        if d.check().is_some() {
            return Err(SerError::InvalidValue(
                "world descriptor failed its consistency check",
            ));
        }
        Ok(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::{from_bytes, to_bytes};
    use cv_determinism::{Quat, Vec3};

    #[test]
    fn placement_prefers_trs_and_falls_back_to_affine() {
        let t = Transform::new(
            Vec3::new(1.0, 2.0, 3.0),
            Quat::from_axis_angle(Vec3::Z, 0.5),
            Vec3::new(2.0, 2.0, 2.0),
        );
        // A TRS matrix decomposes back to the easy form.
        assert!(matches!(
            Placement::from_matrix(Mat4::from(t)),
            Placement::Trs(_)
        ));

        // A sheared one cannot, and is preserved exactly rather than approximated.
        let sheared = Mat4::from(Transform::from_scale(Vec3::new(2.0, 1.0, 1.0)))
            * Mat4::from(Transform::from_rotation(Quat::from_axis_angle(
                Vec3::Z,
                0.7,
            )));
        let p = Placement::from_matrix(sheared);
        assert!(matches!(p, Placement::Affine(_)));
        assert!(p.as_transform().is_none());
        let probe = Vec3::new(1.0, 0.0, 0.0);
        assert!(p
            .transform_point(probe)
            .approx_eq(sheared.transform_point(probe), 1e-12));
    }

    #[test]
    fn mirroring_is_detected_in_both_forms() {
        // A mirror stays in the TRS form, as a negative scale — hosts still need the winding flag.
        let mirrored = Placement::Trs(Transform::from_scale(Vec3::new(-1.0, 1.0, 1.0)));
        assert!(mirrored.is_mirroring());
        assert!(!Placement::IDENTITY.is_mirroring());
        // Two negative axes is a rotation, not a mirror.
        assert!(!Placement::Trs(Transform::from_scale(Vec3::new(-1.0, -1.0, 1.0))).is_mirroring());
        assert!(Placement::Trs(Transform::from_scale(Vec3::splat(-1.0))).is_mirroring());
        // And it agrees with the matrix form.
        assert!(Placement::from_matrix(Mat4::from_reflection(Vec3::X)).is_mirroring());
    }

    fn graph() -> NodeGraph {
        let mut g = NodeGraph::new(1.0, 42);
        let world = g.root();
        let reach = g.add_child(world, "reach").unwrap();
        let area = g.add_child(reach, "area").unwrap();
        let a = g.add_child(area, "space_a").unwrap();
        let b = g.add_child(area, "space_b").unwrap();
        g.connect(a, b).unwrap();
        for h in [world, reach, area, a, b] {
            g.set_envelope(h, Aabb::new(Vec3::ZERO, Vec3::splat(10.0)))
                .unwrap();
            g.advance(h, NodeState::Realized).unwrap();
        }
        g
    }

    #[test]
    fn flattening_is_depth_first_with_parents_first() {
        let g = graph();
        let d = DescriptorBuilder::new(&g, Fingerprint::from_raw(7), 42).finish();
        assert!(d.check().is_none(), "{:?}", d.check());

        let names: Vec<&str> = d.scopes.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["World", "reach", "area", "space_a", "space_b"]);
        assert_eq!(d.root().unwrap().kind, NodeKind::World);
        assert_eq!(d.scopes[0].parent, None);
        // Every parent precedes its child, so a host can build top-down in one pass.
        for (i, s) in d.scopes.iter().enumerate() {
            if let Some(p) = s.parent {
                assert!(p.index() < i);
            }
        }
        // Adjacency survives as indices.
        assert_eq!(d.scopes[3].neighbors, vec![ScopeRef(4)]);
        assert_eq!(d.scopes[4].neighbors, vec![ScopeRef(3)]);
    }

    #[test]
    fn queries_walk_the_flattened_tree() {
        let g = graph();
        let d = DescriptorBuilder::new(&g, Fingerprint::from_raw(7), 42).finish();
        let area = ScopeRef(2);
        let kids: Vec<&str> = d.children_of(area).map(|(_, s)| s.name.as_str()).collect();
        assert_eq!(kids, vec!["space_a", "space_b"]);
        assert_eq!(d.scopes_of_kind(NodeKind::Space).count(), 2);
        assert_eq!(d.realized_scopes().count(), 5);
    }

    #[test]
    fn descriptor_round_trips() {
        let g = graph();
        let mut b = DescriptorBuilder::new(&g, Fingerprint::from_raw(0xABC), 42);
        b.place(InstanceRecord {
            id: ObjectId::derived("instance", "door_1"),
            content: ObjectId::derived("actor", "door"),
            scope: ScopeRef(3),
            placement: Placement::Trs(Transform::from_translation(Vec3::new(1.0, 2.0, 3.0))),
            rationale: Rationale::detailed(PlacementReason::SolverRequired, "gate on edge a→b"),
        });
        b.place_mesh(MeshRecord {
            id: ObjectId::derived("meshinst", "door_1_mesh"),
            mesh: ObjectId::derived("mesh", "kit/door_a"),
            scope: ScopeRef(3),
            placement: Placement::Trs(Transform::from_scale(Vec3::new(-1.0, 1.0, 1.0))),
            collision: vec![Aabb::new(Vec3::ZERO, Vec3::new(1.0, 0.2, 2.0))],
            sockets: vec![Socket {
                name: "hinge".into(),
                transform: Transform::IDENTITY,
            }],
            tags: vec![ObjectId::derived("surface", "portalable")],
            rationale: Rationale::new(PlacementReason::Connector),
        });
        let d = b.finish();

        let bytes = to_bytes(&d);
        let back: WorldDescriptor = from_bytes(&bytes).unwrap();
        assert_eq!(back, d);
        assert_eq!(to_bytes(&back), bytes);
        assert!(
            back.meshes[0].needs_winding_flip(),
            "mirrored placement must be flagged"
        );
        assert!(back.meshes[0].socket("hinge").is_some());
        assert_eq!(back.instances_in(ScopeRef(3)).count(), 1);
        assert_eq!(back.meshes_in(ScopeRef(3)).count(), 1);
    }

    #[test]
    fn a_malformed_descriptor_is_rejected_at_load() {
        let g = graph();
        let mut d = DescriptorBuilder::new(&g, Fingerprint::from_raw(1), 1).finish();
        d.instances.push(InstanceRecord {
            id: ObjectId::from_raw(1),
            content: ObjectId::from_raw(2),
            scope: ScopeRef(999), // out of range
            placement: Placement::IDENTITY,
            rationale: Rationale::new(PlacementReason::Dressing),
        });
        assert!(d.check().is_some());
        assert!(from_bytes::<WorldDescriptor>(&to_bytes(&d)).is_err());
    }

    #[test]
    fn unresolved_content_is_reported_once_not_per_record() {
        use crate::content::ContentKind;
        let g = graph();
        let mut registry = ContentRegistry::new();
        let known = registry.register(ContentKind::Actor, "door", 1).unwrap();

        let mut b = DescriptorBuilder::new(&g, Fingerprint::from_raw(1), 1);
        for i in 0..3 {
            b.place(InstanceRecord {
                id: ObjectId::derived("instance", &format!("known_{i}")),
                content: known,
                scope: ScopeRef(3),
                placement: Placement::IDENTITY,
                rationale: Rationale::new(PlacementReason::Scheduled),
            });
            b.place(InstanceRecord {
                id: ObjectId::derived("instance", &format!("ghost_{i}")),
                content: ObjectId::derived("actor", "missing"),
                scope: ScopeRef(3),
                placement: Placement::IDENTITY,
                rationale: Rationale::new(PlacementReason::Scheduled),
            });
        }
        let missing = b.finish().unresolved_content(&registry);
        assert_eq!(missing, vec![ObjectId::derived("actor", "missing")]);
    }

    #[test]
    fn a_mesh_record_carries_no_geometry() {
        // The whole point of the boundary: a record's size is bounded by its *metadata*, so a
        // cathedral costs the same as a crate. If vertex data ever crept in, this would balloon.
        let record = MeshRecord {
            id: ObjectId::from_raw(1),
            mesh: ObjectId::from_raw(2),
            scope: ScopeRef(0),
            placement: Placement::IDENTITY,
            collision: vec![Aabb::new(Vec3::ZERO, Vec3::ONE)],
            sockets: vec![Socket {
                name: "a".into(),
                transform: Transform::IDENTITY,
            }],
            tags: vec![ObjectId::from_raw(3)],
            rationale: Rationale::new(PlacementReason::Dressing),
        };
        let mut w = Writer::new();
        w.write(&record);
        let size = w.finish().len();
        assert!(
            size < 400,
            "a mesh record should be metadata-sized, got {size} bytes"
        );
    }

    #[test]
    fn rationale_reads_well_and_marks_load_bearing_placements() {
        assert_eq!(
            Rationale::detailed(PlacementReason::SolverRequired, "gate on edge a→b").to_string(),
            "solver-required: gate on edge a→b"
        );
        assert_eq!(
            Rationale::new(PlacementReason::Dressing).to_string(),
            "dressing"
        );
        assert!(PlacementReason::SolverRequired.is_required());
        assert!(PlacementReason::Connector.is_required());
        assert!(!PlacementReason::Dressing.is_required());
    }
}
