// GENERATED — do not edit.
//
// Source: manifest/tier1.toml
// Regenerate: cargo xtask generate
//
// Every edit here is lost on the next run. Change the manifest instead — it is the
// only file in this system that is authored by hand.


/**
 * A picked CLASS path — `Kind<T>` in the manifest.
 *
 * Branded so TypeScript keeps it distinct from an instance reference. Both are a path at runtime,
 * and letting them mix would erase the class-versus-instance distinction the whole authoring model
 * rests on.
 */
export type ClassPath<T> = string & { readonly __class?: T };

/** A live INSTANCE reference — `Ref<T>` in the manifest. */
export type InstanceRef<T> = string & { readonly __instance?: T };

/** A FILE on disk — `Resource<T>` in the manifest, paired with an asset path. */
export type AssetRef<T> = string & { readonly __asset?: T };

/** Metadata values. A closed set: anything outside it does not survive the seam. */
export type MetaValue =
  | boolean
  | number
  | string
  | Vec3
  | Transform
  | MetaValue[]
  | Record<string, MetaValue>;

/**
 * The root of everything with identity. Has metadata, has a rationale, can be asked what it is.
 */
export interface Object {
  /**
   * Opaque identity. A string or opaque handle — NEVER a u64, because TS integers alias past
   * 2^53.
   */
  readonly id: string;
  /**
   * The mount-pointed class path this object was instantiated from.
   */
  readonly type_path: string;
  /**
   * Why the core did what it did with this object. Core-written.
   */
  readonly rationale: InstanceRef<Rationale>;
  /**
   * Every metadata key currently set.
   */
  readonly meta_keys: string[];
  /**
   * Class-membership test against a picked class path.
   */
  is_a(kind: ClassPath<Object>): boolean;
  /**
   * Identity comparison. Never a float comparison.
   */
  equals(other: InstanceRef<Object>): boolean;
  /**
   * Short display form.
   * @default TypeName#id
   */
  format(): string;
  /**
   * Prose a developer reads in the trace.
   * @default format()
   */
  explain(): string;
  /**
   * Read one metadata value.
   */
  meta(key: string): MetaValue;
  /**
   * Write one metadata value. The CV_ prefix is reserved for the core and rejected here.
   */
  set_meta(key: string, value: MetaValue): void;
  /**
   * Is this key set?
   */
  has_meta(key: string): boolean;
  /**
   * Clear one metadata value.
   */
  remove_meta(key: string): void;
  /**
   * Setup phase 1. Own configuration only; other content may not exist yet.
   * @remarks A hook — a question the core asks.
   */
  _configure(): void;
  /**
   * Setup phase 2. Every registered content class now exists and may be referenced.
   * @remarks A hook — a question the core asks.
   */
  _resolve(): void;
}

/**
 * Something placeable in a world. The only thing the core places, and the sole point of contact
 * between content and generation.
 */
export interface Actor extends Object {
  /**
   * Attached components, in attach order — which is the order every aggregating hook walks.
   */
  readonly components: InstanceRef<Component>[];
  /**
   * The actor this one is mounted on, if any.
   */
  readonly parent: InstanceRef<Actor>;
  /**
   * Actors mounted on this one.
   */
  readonly children: InstanceRef<Actor>[];
  /**
   * Local transform relative to the parent.
   */
  transform: Transform;
  /**
   * Whether alternative routes past this actor's gate are tolerated, reported, or actively
   * forbidden. Inert unless gate() is non-trivial.
   * @default TOLERATED
   */
  skip_policy: SkipPolicy;
  /**
   * The declared likelihood a player finds this gate's solution unprompted. Declared because it
   * is not derivable; above the threshold the solver treats the gate as open, conservatively,
   * and names it in the trace.
   * @default 1.0
   */
  discoverability: number;
  /**
   * First attached component of that kind.
   */
  component(kind: ClassPath<Component>): InstanceRef<Component>;
  /**
   * Every attached component of that kind.
   */
  components_of(kind: ClassPath<Component>): InstanceRef<Component>[];
  /**
   * Attach a component.
   */
  add_component(c: InstanceRef<Component>): void;
  /**
   * Detach a component.
   */
  remove_component(c: InstanceRef<Component>): void;
  /**
   * Mount this actor onto another at a named socket.
   */
  attach_to(parent: InstanceRef<Actor>, mount: InstanceRef<MountComponent>): void;
  /**
   * Absolute transform. Walks the parent chain, so it is not free.
   */
  world_transform(): Transform;
  /**
   * The point this actor rotates and mounts about.
   * @default Vec3.ZERO
   * @remarks A hook — a question the core asks.
   */
  pivot(ctx: InstanceRef<Context>): Vec3;
  /**
   * Axis-aligned extent.
   * @default union of component bounds
   * @remarks A hook — a question the core asks.
   */
  bounds(ctx: InstanceRef<Context>): Aabb;
  /**
   * The space this reserves during skeleton layout.
   * @default union of component collision
   * @remarks A hook — a question the core asks.
   */
  footprint(ctx: InstanceRef<Context>): InstanceRef<CollisionBody>;
  /**
   * Space that must stay empty around this actor. Called per candidate position, so it must stay
   * cheap.
   * @default empty
   * @remarks A hook — a question the core asks.
   */
  clearance(ctx: InstanceRef<Context>): InstanceRef<CollisionBody>;
  /**
   * What this actor physically blocks.
   * @default union of component collision
   * @remarks A hook — a question the core asks.
   */
  collision(ctx: InstanceRef<Context>): InstanceRef<CollisionBody>;
  /**
   * Which faces this may be mounted against. Floor, wall or ceiling is what distinguishes a
   * chest from a sconce from a chandelier.
   * @default [ POS_Y ]
   * @remarks A hook — a question the core asks.
   */
  mount_faces(ctx: InstanceRef<Context>): Face[];
  /**
   * Which direction is up for this actor.
   * @default Vec3.UP
   * @remarks A hook — a question the core asks.
   */
  up_axis(ctx: InstanceRef<Context>): Vec3;
  /**
   * Rotational quantisation about the up axis. 0 = free, 4 = 90 degrees.
   * @default 4
   * @remarks A hook — a question the core asks.
   */
  yaw_snap(ctx: InstanceRef<Context>): number;
  /**
   * May the solver tilt this out of the up axis?
   * @default false
   * @remarks A hook — a question the core asks.
   */
  allow_pitch(ctx: InstanceRef<Context>): boolean;
  /**
   * May the solver roll this about its forward axis?
   * @default false
   * @remarks A hook — a question the core asks.
   */
  allow_roll(ctx: InstanceRef<Context>): boolean;
  /**
   * Permitted uniform scale range. A lever a fixed-template system cannot have; convexity is
   * affine-invariant, so the collision cache survives it.
   * @default Span(1, 1)
   * @remarks A hook — a question the core asks.
   */
  scalable(ctx: InstanceRef<Context>): Span;
  /**
   * Forbid non-uniform scaling.
   * @default true
   * @remarks A hook — a question the core asks.
   */
  uniform_scale_only(ctx: InstanceRef<Context>): boolean;
  /**
   * How far this may sink into the surface it mounts on.
   * @default Span(0, 0)
   * @remarks A hook — a question the core asks.
   */
  embed_depth(ctx: InstanceRef<Context>): Span;
  /**
   * Hard placement constraints. A door names its own unlock here, which is where key-to-lock
   * distance is written.
   * @default []
   * @remarks A hook — a question the core asks.
   */
  constraints(ctx: InstanceRef<Context>): InstanceRef<Constraint>[];
  /**
   * Soft placement biases. Relaxable, and reported when relaxed.
   * @default []
   * @remarks A hook — a question the core asks.
   */
  preferences(ctx: InstanceRef<Context>): InstanceRef<Preference>[];
  /**
   * Which roles this actor may be assigned. Role is an output; this only narrows the candidate
   * set.
   * @default all roles
   * @remarks A hook — a question the core asks.
   */
  eligible_roles(ctx: InstanceRef<Context>): Role[];
  /**
   * Ordering rules relative to other content.
   * @default []
   * @remarks A hook — a question the core asks.
   */
  schedule(ctx: InstanceRef<Context>): InstanceRef<ScheduleRule>[];
  /**
   * How many of this may exist, per scope.
   * @remarks A hook — a question the core asks.
   */
  quota(ctx: InstanceRef<Context>): Quota;
  /**
   * Selection bias among eligible candidates.
   * @default 1.0
   * @remarks A hook — a question the core asks.
   */
  weight(ctx: InstanceRef<Context>): number;
  /**
   * How strongly instances of this push away from each other. A pressure, never an obligation.
   * @default 0.0
   * @remarks A hook — a question the core asks.
   */
  repulsion(ctx: InstanceRef<Context>): number;
  /**
   * What can be done here.
   * @default aggregate enabled components in attach order
   * @remarks A hook — a question the core asks.
   */
  enables(ctx: InstanceRef<Context>): InstanceRef<Interaction>[];
  /**
   * What must exist near me for my mechanic to work. This is the hook that makes the generator
   * place enabling content.
   * @default aggregate enabled components in attach order
   * @remarks A hook — a question the core asks.
   */
  requires(ctx: InstanceRef<Context>): InstanceRef<PlacementNeed>[];
  /**
   * What must NOT be near me. The negative half of an obligation.
   * @default aggregate enabled components in attach order
   * @remarks A hook — a question the core asks.
   */
  forbids(ctx: InstanceRef<Context>): InstanceRef<Exclusion>[];
  /**
   * Is this proposal good, and how wrong is it? The magnitude is the point: a boolean turns the
   * placement search into a reroll.
   * @default AcceptedVerdict
   * @remarks A hook — a question the core asks.
   */
  judge(ctx: InstanceRef<Context>, path: InstanceRef<Path>): InstanceRef<Verdict>;
  /**
   * What the occupant must hold to pass.
   * @default AlwaysRule
   * @remarks A hook — a question the core asks.
   */
  gate(ctx: InstanceRef<Context>): InstanceRef<Rule>;
  /**
   * Is this dangerous, how much, and is it avoidable?
   * @default Harm.NONE
   * @remarks A hook — a question the core asks.
   */
  harm(ctx: InstanceRef<Context>): Harm;
  /**
   * Unlocks the occupant keeps after reaching this. ROWS of an UnlockTableResource, never
   * classes — the lattice is over identities and a row id already is one. An unlock carries NO
   * behaviour: every mechanical consequence belongs to a Component.
   * @default aggregate enabled components in attach order
   * @remarks A hook — a question the core asks.
   */
  grants(ctx: InstanceRef<Context>): Unlock[];
  /**
   * Event: the solver is considering this position.
   * @remarks A hook — a question the core asks.
   */
  on_proposed(ctx: InstanceRef<Context>): void;
  /**
   * Event: this actor was committed to a position.
   * @remarks A hook — a question the core asks.
   */
  on_placed(ctx: InstanceRef<Context>): void;
  /**
   * Event: a candidate position was refused, with the verdict that refused it.
   * @remarks A hook — a question the core asks.
   */
  on_rejected(ctx: InstanceRef<Context>, why: InstanceRef<Verdict>): void;
  /**
   * Event: an occupant reached this and took what it grants.
   * @remarks A hook — a question the core asks.
   */
  on_obtained(ctx: InstanceRef<Context>): void;
  /**
   * Event: an attached component invalidated itself.
   * @remarks A hook — a question the core asks.
   */
  on_component_changed(c: InstanceRef<Component>): void;
  /**
   * Event: generation is complete and this actor is realized.
   * @remarks A hook — a question the core asks.
   */
  on_finalized(ctx: InstanceRef<Context>): void;
  /**
   * Event: the solver is backtracking past this actor. Lets a rewind be clean.
   * @remarks A hook — a question the core asks.
   */
  reset(ctx: InstanceRef<Context>): void;
}

/**
 * An obtainable actor. The thing that hands out unlocks, as distinct from the unlocks themselves.
 */
export interface Item extends Actor {
  /**
   * What kind of reward this is. An INPUT the developer declares, never an output — and it
   * defaults to the conservative answer.
   * @default PROGRESSION
   * @remarks A hook — a question the core asks.
   */
  classification(ctx: InstanceRef<Context>): ItemClass;
  /**
   * How many, and whether taking it consumes it.
   * @default Quantity(1, 1, false)
   * @remarks A hook — a question the core asks.
   */
  quantity(ctx: InstanceRef<Context>): Quantity;
  /**
   * Whether and how this comes back.
   * @default NEVER
   * @remarks A hook — a question the core asks.
   */
  replenishes(ctx: InstanceRef<Context>): Replenish;
}

/**
 * An attachable behaviour with its own transform. The seven hooks have the same signatures as
 * Actor, and this is where a mechanic is written.
 */
export interface Component extends Object {
  /**
   * The actor this is attached to.
   */
  readonly owner: InstanceRef<Actor>;
  /**
   * Disabled components are skipped by every aggregating hook.
   * @default true
   */
  enabled: boolean;
  /**
   * Local transform relative to the owner.
   * @default identity
   */
  transform: Transform;
  /**
   * Absolute transform. Walks the parent chain, so it is not free.
   */
  world_transform(): Transform;
  /**
   * Signal that this component changed; the owner receives on_component_changed.
   */
  invalidate(): void;
  /**
   * Event: attached to an actor.
   * @remarks A hook — a question the core asks.
   */
  on_attached(owner: InstanceRef<Actor>): void;
  /**
   * Event: detached from an actor.
   * @remarks A hook — a question the core asks.
   */
  on_detached(): void;
  /**
   * Editor-time self-check. Never blocks generation.
   * @default []
   * @remarks A hook — a question the core asks.
   */
  validate(): Diagnostic[];
  /**
   * Whether this component's supply comes back. Not only Items: a dispenser and an environmental
   * source are not Items.
   * @default NEVER
   * @remarks A hook — a question the core asks.
   */
  replenishes(ctx: InstanceRef<Context>): Replenish;
}

/**
 * Imported geometry, and the material-name to Surface mapping that gives it meaning.
 */
export interface MeshComponent extends Component {
  /**
   * The mesh file this draws and derives collision from.
   */
  asset: AssetRef<MeshResource>;
  /**
   * Submesh or material name to Surface class. This is how imported art acquires generation
   * meaning.
   */
  surfaces: Record<string, ClassPath<Surface>>;
  /**
   * How collision is derived from the mesh.
   */
  collision_mode: CollisionMode;
  /**
   * Visible is not the same as collidable.
   * @default true
   */
  visible: boolean;
  /**
   * The derived collision body.
   */
  collision(): InstanceRef<CollisionBody>;
}

/**
 * A parametric shape. Collision is computed analytically from its parameters, never from
 * tessellation.
 */
export interface ShapeComponent extends Component {
  /**
   * The parametric primitive.
   */
  shape: Shape;
  /**
   * The Surface class this shape presents.
   */
  surface: ClassPath<Surface>;
  /**
   * How collision is derived.
   */
  collision_mode: CollisionMode;
  /**
   * Visible is not the same as collidable.
   * @default true
   */
  visible: boolean;
  /**
   * The analytically derived collision body.
   */
  collision(): InstanceRef<CollisionBody>;
}

/**
 * An attachment socket. What may mount here is a tag query, not a hand-maintained class list.
 */
export interface MountComponent extends Component {
  /**
   * A label for a developer reading the viewport. Deliberately not a lookup key.
   */
  name: string;
  /**
   * Which content may mount here.
   */
  accepts: TagQuery;
  /**
   * Which faces of the mounted thing may meet this socket.
   */
  faces: Face[];
  /**
   * Space that must stay empty around whatever mounts here.
   */
  clearance: InstanceRef<CollisionBody>;
  /**
   * Whether a specific candidate may mount, beyond the tag query.
   * @default AlwaysRule
   * @remarks A hook — a question the core asks.
   */
  admits(ctx: InstanceRef<Context>, candidate: InstanceRef<Actor>): InstanceRef<Rule>;
}

/**
 * Turns a spatial delta into a directed graph edge. The thing that makes a staircase a route
 * rather than scenery.
 */
export interface TraversalComponent extends Component {
  /**
   * Horizontal distance this move covers.
   * @remarks A hook — a question the core asks.
   */
  run(ctx: InstanceRef<Context>): Span;
  /**
   * Vertical delta this move covers. Negative is downward.
   * @remarks A hook — a question the core asks.
   */
  rise(ctx: InstanceRef<Context>): Span;
  /**
   * Permitted directions as a cone, so 'up, level or diagonally up, never down' is expressible.
   * @remarks A hook — a question the core asks.
   */
  direction(ctx: InstanceRef<Context>): DirectionCone;
  /**
   * What the occupant must hold to make this move, IN THIS DIRECTION — so one edge can be open
   * one way and gated the other.
   * @default AlwaysRule
   * @remarks A hook — a question the core asks.
   */
  admits(ctx: InstanceRef<Context>, occupant: Occupant, dir: Vec3): InstanceRef<Rule>;
  /**
   * What traversing this spends against a route budget.
   * @default 1.0
   * @remarks A hook — a question the core asks.
   */
  cost(ctx: InstanceRef<Context>): number;
  /**
   * What the occupant needs at the near end before the move is possible.
   * @remarks A hook — a question the core asks.
   */
  approach(ctx: InstanceRef<Context>): Approach;
  /**
   * PROPOSED. The swept volume that must be empty for the move to exist. run and rise describe
   * only the endpoints, and admit a jump the real arc would not clear under a low ceiling.
   * @default the box implied by run x rise
   * @remarks A hook — a question the core asks.
   * @experimental PROPOSED — may change or be removed.
   */
  clearance(ctx: InstanceRef<Context>): InstanceRef<CollisionBody>;
}

/**
 * A place that returns the world to a known-good state. P15's second satisfaction route, and what
 * lets the solver take an attractive one-way transition instead of refusing every irreversible
 * edge.
 */
export interface CheckpointComponent extends Component {
  /**
   * Which CLASSES OF PLACED CONTENT respawn here -- consumables, destructibles, enemies. NOT
   * unlocks: an unlock is monotone and can never be lost, so restoring one has no meaning.
   */
  restores: ClassPath<Object>[];
  /**
   * Whether the occupant also returns here — which is what makes this a respawn point as well.
   * @default false
   */
  restores_occupant: boolean;
  /**
   * How far this checkpoint's effect reaches.
   */
  scope: InstanceScope;
}

/**
 * A node in a travel network. Not cosmetic: a network collapses traversal cost across the whole
 * World, and difficulty here IS slack spent against a budget.
 */
export interface FastTravelComponent extends Component {
  /**
   * Nodes sharing a network name connect to each other.
   */
  network: string;
  /**
   * What one hop spends — in the same currency as the routes it competes with.
   */
  cost: BudgetRef;
  /**
   * What the occupant must hold before this node joins its network.
   * @default AlwaysRule
   */
  unlocked_by: InstanceRef<Rule>;
}

/**
 * Writes a world-state variable. The authoring surface for the non-monotone axis.
 */
export interface StateSetterComponent extends Component {
  /**
   * Which declared state variable this writes.
   */
  variable: string;
  /**
   * The value it writes.
   */
  to_value: string;
  /**
   * If set, the value holds only WHILE something carrying this component is present — which is
   * what makes dwell an ordinary reading rather than a trigger kind.
   */
  while_occupied_by: ClassPath<Component>;
}

/**
 * Place me on an edge of this kind, and close it. Without this a barrier can only be authored as
 * geometry, which violates gate-never-delete by construction.
 */
export interface BlocksTraversalComponent extends Component {
  /**
   * Which kind of traversal edge this may be placed on.
   */
  matching: ClassPath<TraversalComponent>;
  /**
   * The route this blocker obliges or forbids.
   */
  route: InstanceRef<Route>;
}

/**
 * What a piece of geometry means to a mechanic. Answers who may be here and what works here.
 */
export interface Surface extends Object {
  /**
   * Display name.
   */
  name: string;
  /**
   * Why an eligible-surface list is a TagQuery rather than a hand-maintained class list. The
   * editor must show these on selected geometry.
   */
  tags: Tag[];
  /**
   * Can something BE here? A state you are in, evaluated per occupant and per sphere.
   * @default [ Support(AlwaysRule) ]
   * @remarks A hook — a question the core asks.
   */
  supports(ctx: InstanceRef<Context>, occupant: Occupant): Support[];
  /**
   * Does an ACTION work here? Covers transit THROUGH as well as action UPON, which is how one
   * pane of glass blocks ballistics and walking while passing a laser and a sightline.
   * @default AlwaysRule
   * @remarks A hook — a question the core asks.
   */
  affords(ctx: InstanceRef<Context>, attempt: InstanceRef<Interaction>): InstanceRef<Rule>;
  /**
   * Traversal resistance.
   * @default 1.0
   * @remarks A hook — a question the core asks.
   */
  friction(ctx: InstanceRef<Context>): number;
  /**
   * Is standing here dangerous?
   * @default Harm.NONE
   * @remarks A hook — a question the core asks.
   */
  harm(ctx: InstanceRef<Context>): Harm;
  /**
   * May something mount against this surface?
   * @default AlwaysRule
   * @remarks A hook — a question the core asks.
   */
  admits_mount(ctx: InstanceRef<Context>, m: InstanceRef<MountComponent>): InstanceRef<Rule>;
}

/**
 * Sealed on purpose: the solver walks this tree as the analysable half of a gate, so a
 * developer-defined rule would make the dependency walk impossible.
 *
 * Sealed: content may not subclass this.
 */
export interface Rule extends Object {
  /**
   * Does this rule currently hold?
   */
  is_open(): boolean;
  /**
   * Every unlock this rule mentions. The solver's dependency walk, and what lets requires()
   * plant a source.
   */
  referenced(): Unlock[];
  /**
   * Prose for a lock badge and the trace.
   */
  explain(): string;
}

/**
 * Trivially satisfied. The identity of AllOfRule.
 *
 * Sealed: content may not subclass this.
 */
export interface AlwaysRule extends Rule {
}

/**
 * Never satisfied. The identity of AnyOfRule. On a sole route this is an authoring error and is
 * reported with the numbers, not silently accepted.
 *
 * Sealed: content may not subclass this.
 */
export interface NeverRule extends Rule {
}

/**
 * The occupant holds this unlock, or anything that supersedes it.
 *
 * Sealed: content may not subclass this.
 */
export interface HoldsRule extends Rule {
  /**
   * The unlock required. Satisfied by this row, or by any held unlock whose supersedes closure
   * contains it.
   */
  unlock: Unlock;
  /**
   * How many. Quantity lives here, not in a number of instances.
   * @default 1
   */
  count: number;
}

/**
 * The occupant holds something CARRYING a matching component. A DIFFERENT question from HoldsRule:
 * this asks about the things held, that asks about the lattice.
 *
 * Sealed: content may not subclass this.
 */
export interface HasComponentRule extends Rule {
  /**
   * The component class required.
   */
  kind: ClassPath<Component>;
}

/**
 * Every child holds.
 *
 * Sealed: content may not subclass this.
 */
export interface AllOfRule extends Rule {
  /**
   * The conjuncts.
   */
  children: InstanceRef<Rule>[];
}

/**
 * At least one child holds. This is what proves genuine alternate routes rather than one route the
 * solver happened to pick.
 *
 * Sealed: content may not subclass this.
 */
export interface AnyOfRule extends Rule {
  /**
   * The disjuncts.
   */
  children: InstanceRef<Rule>[];
}

/**
 * The inner rule does NOT hold. Non-monotonic, so it is accepted and reported rather than refused.
 *
 * Sealed: content may not subclass this.
 */
export interface NegateRule extends Rule {
  /**
   * The negated rule.
   */
  inner: InstanceRef<Rule>;
}

/**
 * Something matching is accessible within range of this gate. The contextual half: a gate may
 * depend on the WORLD rather than only the occupant, and because the dependency walk sees it,
 * requires() can plant the source.
 *
 * Sealed: content may not subclass this.
 */
export interface NearbyRule extends Rule {
  /**
   * What must be nearby.
   */
  kind: ClassPath<Object>;
  /**
   * How near, in whatever currency the budget names.
   */
  within: BudgetRef;
  /**
   * The scope searched.
   */
  scope: InstanceScope;
}

/**
 * What judge() returns. A boolean would tell the solver nothing and turn the placement search into
 * a reroll.
 *
 * Sealed: content may not subclass this.
 */
export interface Verdict extends Object {
}

/**
 * It fits. The slack says how comfortably, which feeds difficulty.
 *
 * Sealed: content may not subclass this.
 */
export interface AcceptedVerdict extends Verdict {
  /**
   * Budget remaining after this placement.
   */
  slack: number;
}

/**
 * Too far. Move the target closer by roughly this much — a direction the solver can act on.
 *
 * Sealed: content may not subclass this.
 */
export interface OverBudgetVerdict extends Verdict {
  /**
   * How much over budget.
   */
  excess: number;
  /**
   * Which budget it was measured against. 'Over budget by 6.2' leaves a developer guessing;
   * 'over budget by 6.2 against grapple reach' names the lever to pull. Null only where the
   * comparison was against a bare number.
   */
  against: InstanceRef<Budget>;
}

/**
 * Something is in the way. Remove or reposition that, or route around it.
 *
 * Sealed: content may not subclass this.
 */
export interface BlockedVerdict extends Verdict {
  /**
   * What blocked it.
   */
  by: InstanceRef<Object>;
}

/**
 * Wrong kind of thing. Do not retry this candidate.
 *
 * Sealed: content may not subclass this.
 */
export interface UnsuitableVerdict extends Verdict {
  /**
   * Prose for the trace.
   */
  reason: string;
}

/**
 * Something an occupant attempts. Subclassed by what relocates, which is what lets one surface
 * answer four mechanics differently.
 */
export interface Interaction extends Object {
  /**
   * How far this reaches. NOT named 'reach' — Reach is a scope.
   */
  range: Span;
  /**
   * Whether an unobstructed line is required.
   * @default false
   */
  line_of_sight: boolean;
  /**
   * Whether the occupant must be supported to attempt this.
   * @default true
   */
  from_standing: boolean;
  /**
   * What performing this spends. What consumes a resource is the consumer, not the consumable.
   * @default []
   * @remarks A hook — a question the core asks.
   */
  consumes(): Cost[];
  /**
   * What the occupant must hold to perform this. Deliberately the same name and meaning as
   * Actor.gate, and deliberately NOT 'requires', which on an Actor means something unrelated.
   * @default AlwaysRule
   * @remarks A hook — a question the core asks.
   */
  gate(): InstanceRef<Rule>;
  /**
   * Prose for the trace.
   */
  explain(): string;
}

/**
 * THE PLAYER relocates, so this is a directed graph edge.
 */
export interface Movement extends Interaction {
  /**
   * Vertical delta.
   */
  rise: Span;
  /**
   * Permitted directions.
   */
  direction: DirectionCone;
}

/**
 * AN OBJECT relocates, not the player. Push, carry and throw are authored subclasses of this.
 */
export interface Displace extends Interaction {
  /**
   * What moves.
   */
  subject: InstanceRef<Object>;
}

/**
 * NOTHING relocates — it acts at range. Sightlines, beams and ballistics are authored
 * subclasses, which is how transit through a surface is expressed without a core enum.
 */
export interface RemoteUse extends Interaction {
  /**
   * What it acts upon.
   */
  target: ClassPath<Object>;
}

/**
 * A parametric primitive. Collision is computed from parameters, never from tessellation —
 * otherwise a visual LOD change silently alters generation.
 */
export interface Shape {
  /**
   * Axis-aligned extent.
   */
  bounds(): Aabb;
  /**
   * Enclosed volume.
   */
  volume(): number;
  /**
   * Convexity, which decides whether the collision cache survives scaling.
   */
  is_convex(): boolean;
  /**
   * Point containment.
   */
  contains(p: Vec3): boolean;
  /**
   * Nearest surface point.
   */
  closest_point(p: Vec3): Vec3;
  /**
   * Convex decomposition.
   */
  decompose(): Shape[];
  /**
   * Render geometry. Deliberately NOT the source of collision.
   */
  tessellate(lod: number): InstanceRef<MeshResource>;
}

/**
 * The forms of Shape. Switch on `form` — TypeScript narrows each arm.
 */
export type ShapeForm =
  | (SolidShape & { form: "SolidShape" })
  | (SurfaceShape & { form: "SurfaceShape" });

/**
 * A shape with an inside. Supports booleans and a signed distance field.
 */
export interface SolidShape extends Shape {
  /**
   * Signed distance to the surface.
   */
  sdf(p: Vec3): number;
}

/**
 * The forms of SolidShape. Switch on `form` — TypeScript narrows each arm.
 */
export type SolidShapeForm =
  | (CubeShape & { form: "CubeShape" })
  | (SphereShape & { form: "SphereShape" })
  | (HemisphereShape & { form: "HemisphereShape" })
  | (ConeShape & { form: "ConeShape" })
  | (CapsuleShape & { form: "CapsuleShape" })
  | (CylinderShape & { form: "CylinderShape" })
  | (PrismShape & { form: "PrismShape" })
  | (TorusShape & { form: "TorusShape" })
  | (PipeShape & { form: "PipeShape" })
  | (ArchShape & { form: "ArchShape" })
  | (RampShape & { form: "RampShape" })
  | (StairsShape & { form: "StairsShape" })
  | (SpiralStairsShape & { form: "SpiralStairsShape" })
  | (CompositeShape & { form: "CompositeShape" });

/**
 * Zero thickness — no inside, so no sdf and no booleans.
 */
export interface SurfaceShape extends Shape {
}

/**
 * The forms of SurfaceShape. Switch on `form` — TypeScript narrows each arm.
 */
export type SurfaceShapeForm =
  | (QuadShape & { form: "QuadShape" })
  | (TriangleShape & { form: "TriangleShape" })
  | (DiscShape & { form: "DiscShape" })
  | (EllipseShape & { form: "EllipseShape" });

/**
 * A box.
 */
export interface CubeShape extends SolidShape {
  /**
   * Half-extents.
   */
  extents: Vec3;
  /**
   * Edge bevel.
   * @default 0.0
   */
  bevel: number;
  /**
   * Tessellation density per axis. Render only.
   */
  segments: Vec3;
}

/**
 * A sphere.
 */
export interface SphereShape extends SolidShape {
  /**
   * Radius.
   */
  radius: number;
  /**
   * Tessellation density.
   */
  segments: Vec2;
}

/**
 * Half a sphere.
 */
export interface HemisphereShape extends SolidShape {
  /**
   * Radius.
   */
  radius: number;
  /**
   * Tessellation density.
   */
  segments: Vec2;
  /**
   * Whether the flat face is closed.
   * @default true
   */
  capped: boolean;
}

/**
 * A cone.
 */
export interface ConeShape extends SolidShape {
  /**
   * Base radius.
   */
  radius: number;
  /**
   * Height.
   */
  height: number;
  /**
   * Radial tessellation.
   */
  segments: number;
  /**
   * Whether the base is closed.
   * @default true
   */
  capped: boolean;
}

/**
 * A capsule — the usual occupant proxy.
 */
export interface CapsuleShape extends SolidShape {
  /**
   * Radius.
   */
  radius: number;
  /**
   * Height of the cylindrical section.
   */
  height: number;
  /**
   * Tessellation density.
   */
  segments: Vec2;
}

/**
 * A cylinder, or a truncated cone when the radii differ.
 */
export interface CylinderShape extends SolidShape {
  /**
   * Top radius.
   */
  radius_top: number;
  /**
   * Bottom radius.
   */
  radius_bottom: number;
  /**
   * Height.
   */
  height: number;
  /**
   * Radial tessellation.
   */
  segments: number;
  /**
   * Whether the ends are closed.
   * @default true
   */
  capped: boolean;
}

/**
 * An n-sided prism.
 */
export interface PrismShape extends SolidShape {
  /**
   * Number of sides.
   */
  sides: number;
  /**
   * Circumradius.
   */
  radius: number;
  /**
   * Height.
   */
  height: number;
  /**
   * Rotation applied across the height.
   * @default 0.0
   */
  twist: number;
}

/**
 * A torus, optionally a partial arc.
 */
export interface TorusShape extends SolidShape {
  /**
   * Ring radius.
   */
  major_radius: number;
  /**
   * Tube radius.
   */
  minor_radius: number;
  /**
   * Tessellation density.
   */
  segments: Vec2;
  /**
   * Swept angle. A full torus sweeps the whole circle.
   */
  arc_sweep: number;
}

/**
 * A hollow cylinder.
 */
export interface PipeShape extends SolidShape {
  /**
   * Bore radius.
   */
  inner_radius: number;
  /**
   * Outer radius.
   */
  outer_radius: number;
  /**
   * Height.
   */
  height: number;
  /**
   * Radial tessellation.
   */
  segments: number;
}

/**
 * An arched opening — a doorway primitive.
 */
export interface ArchShape extends SolidShape {
  /**
   * Opening width.
   */
  width: number;
  /**
   * Total height.
   */
  height: number;
  /**
   * Wall thickness.
   */
  depth: number;
  /**
   * Radius of the arch crown.
   */
  arch_radius: number;
  /**
   * Arc tessellation.
   */
  segments: number;
}

/**
 * An inclined plane with thickness. A traversal primitive as much as a visual one.
 */
export interface RampShape extends SolidShape {
  /**
   * Width.
   */
  width: number;
  /**
   * Horizontal distance covered.
   */
  run: number;
  /**
   * Vertical distance covered. With run, this is what max_floor_slope is tested against.
   */
  rise: number;
  /**
   * Slab thickness.
   */
  thickness: number;
  /**
   * Whether the sides are enclosed.
   * @default false
   */
  side_walls: boolean;
}

/**
 * A straight flight. The canonical intra-Space traversal edge.
 */
export interface StairsShape extends SolidShape {
  /**
   * Width.
   */
  width: number;
  /**
   * Step count.
   */
  steps: number;
  /**
   * Height of one step.
   */
  step_rise: number;
  /**
   * Depth of one step.
   */
  step_run: number;
  /**
   * Whether the vertical faces are closed.
   * @default true
   */
  risers: boolean;
  /**
   * Step index a landing interrupts at. 0 = none.
   * @default 0
   */
  landing_at: number;
  /**
   * Depth of that landing.
   * @default 0.0
   */
  landing_run: number;
}

/**
 * A helical flight. The primitive that proves the library: it is the canonical multi-floor
 * traversal AND the canonical is-this-a-ramp-or-steps collision question.
 */
export interface SpiralStairsShape extends SolidShape {
  /**
   * Inner radius.
   */
  inner_radius: number;
  /**
   * Outer radius.
   */
  outer_radius: number;
  /**
   * Total height climbed.
   */
  total_rise: number;
  /**
   * Step count.
   */
  steps: number;
  /**
   * Total swept angle.
   */
  sweep: number;
  /**
   * Handedness.
   * @default true
   */
  clockwise: boolean;
  /**
   * Whether a central column is present.
   * @default true
   */
  center_post: boolean;
}

/**
 * A boolean combination of solid shapes.
 */
export interface CompositeShape extends SolidShape {
  /**
   * The ordered operation list.
   */
  operations: BooleanOp[];
}

/**
 * A flat rectangle.
 */
export interface QuadShape extends SurfaceShape {
  /**
   * Half-extents.
   */
  extents: Vec2;
  /**
   * Tessellation density.
   */
  segments: Vec2;
}

/**
 * A single triangle.
 */
export interface TriangleShape extends SurfaceShape {
  /**
   * First vertex.
   */
  a: Vec3;
  /**
   * Second vertex.
   */
  b: Vec3;
  /**
   * Third vertex.
   */
  c: Vec3;
}

/**
 * A flat disc, optionally a sector.
 */
export interface DiscShape extends SurfaceShape {
  /**
   * Radius.
   */
  radius: number;
  /**
   * Radial tessellation.
   */
  segments: number;
  /**
   * Start angle.
   * @default 0.0
   */
  arc_start: number;
  /**
   * Swept angle.
   */
  arc_sweep: number;
}

/**
 * A flat ellipse.
 */
export interface EllipseShape extends SurfaceShape {
  /**
   * Semi-axes.
   */
  radius: Vec2;
  /**
   * Tessellation density.
   */
  segments: number;
}

/**
 * A set of collision islands. What actually blocks, as distinct from what is drawn.
 */
export interface CollisionBody extends Object {
  /**
   * The disjoint pieces.
   */
  islands(): CollisionData[];
  /**
   * Add an island.
   */
  add(d: CollisionData): void;
  /**
   * Remove an island.
   */
  remove(d: CollisionData): void;
  /**
   * Combined extent.
   */
  bounds(): Aabb;
  /**
   * Whether the whole body is convex.
   */
  is_convex(): boolean;
  /**
   * How far this body diverges from the source geometry. Heat-mapped in the editor to catch a
   * ramp collider left on a spiral staircase.
   */
  fit_error(): number;
}

/**
 * Bytes on disk, content-hashed. Referenced from a schematic as a class plus an asset path, never
 * inlined.
 */
export interface Resource extends Object {
  /**
   * The asset path.
   */
  readonly path: string;
  /**
   * Content hash. Part of the fingerprint, which is what makes an asset swap change the recipe.
   */
  readonly hash: string;
  /**
   * Whether the bytes are resident. A resource may be REFERENCED during the solve without being
   * loaded.
   */
  readonly is_loaded: boolean;
}

/**
 * Imported geometry. In a cooked build this is a hash and a bounds with no triangles behind it.
 */
export interface MeshResource extends Resource {
  /**
   * Extent, available even when cooked.
   */
  readonly bounds: Aabb;
  /**
   * Triangle count.
   */
  readonly triangle_count: number;
  /**
   * Material or submesh names, which MeshComponent maps to Surfaces.
   */
  readonly submesh_names: string[];
  /**
   * Derive collision. In a cooked build this returns the baked body or ERRORS LOUDLY — never a
   * silent empty body, which would turn a shipping bug into a world with no collision.
   */
  derive_collision(mode: CollisionMode): InstanceRef<CollisionBody>;
  /**
   * Write the mesh out.
   */
  export(path: string): void;
}

/**
 * The project's progression vocabulary -- named rows, each one atom of the lattice. JSON, not the
 * block notation, because it has no nodes to notate. A project may hold any number of these files
 * anywhere under /Content; THE FILE IS THE UNIT OF SHARING, so copying it carries the vocabulary
 * with it.
 */
export interface UnlockTableResource extends Resource {
  /**
   * The row names, in file order.
   */
  readonly rows: string[];
  /**
   * One row by display name. Convenience for authoring; by_id is the identity lookup.
   */
  row(name: string): Unlock;
  /**
   * One row by its stable id. IDENTITY IS THE ID, never the name -- renaming a row must rewrite
   * zero references.
   */
  by_id(id: string): Unlock;
}

/**
 * One or more NAMED CURVES over one NAMED DOMAIN AXIS. One resource type where UE has four: a
 * vector curve is three rows, a colour curve is four. JSON, not the block notation, because it has
 * no nodes to notate.
 */
export interface CurveTableResource extends Resource {
  /**
   * The axis rows are read over. Bound BY NAME to whichever ProgressionAxis carries it.
   */
  readonly domain: string;
  /**
   * Editor-only, so the vertical axis has a name.
   */
  readonly y_label: string;
  /**
   * Named curves.
   */
  readonly rows: string[];
  /**
   * One row as a Curve struct.
   */
  row(name: string): Curve;
  /**
   * Read a row at a domain value.
   */
  sample(row: string, x: number): number;
}

/**
 * Supplies the x a curve row is sampled at. Built-ins cover depth, space count, unlock count and
 * sphere; a developer subclasses it for anything else, which is the only way to say 'complexity
 * gains weight each time a boss is placed'.
 */
export interface ProgressionAxis extends Object {
  /**
   * What a CurveTable's domain matches against.
   */
  readonly name: string;
  /**
   * The current position along this axis. No default — a subclass must answer.
   * @remarks A hook — a question the core asks.
   */
  value(ctx: InstanceRef<Context>): number;
}

/**
 * Reach index divided by Reach count.
 */
export interface Depth extends ProgressionAxis {
}

/**
 * How many Spaces exist so far.
 */
export interface SpaceCount extends ProgressionAxis {
}

/**
 * How many progression unlocks are held.
 */
export interface UnlockCount extends ProgressionAxis {
}

/**
 * The current accessibility sphere.
 */
export interface Sphere extends ProgressionAxis {
}

/**
 * A REQUIRED OR FORBIDDEN path with a budget, an occupant and predicates. The sign is in the
 * primitive: a Span DECLARES a range, a Route OBLIGES one.
 */
export interface Route extends Object {
  /**
   * Origin. Null means unbound and resolved by the solver.
   */
  from: InstanceRef<Object>;
  /**
   * Destination. Null means unbound.
   */
  to: InstanceRef<Object>;
  /**
   * What traversing it may spend.
   */
  budget: BudgetRef;
  /**
   * Who traverses. Null means the player.
   */
  occupant: Occupant;
  /**
   * Every listed occupant must traverse — an escort, not a simultaneous hold.
   */
  party: Occupant[];
  /**
   * Maximum separation in metres. 0 = unconstrained.
   * @default 0.0
   */
  cohesion: number;
  /**
   * Whether an unobstructed line is obliged. An OBLIGATION, not an observation — which is what
   * makes L4 keep it.
   * @default false
   */
  line_of_sight: boolean;
  /**
   * Whether the origin must be supported.
   * @default true
   */
  from_standing: boolean;
  /**
   * Content this route must not pass. The negative half.
   */
  forbidden: ClassPath<Object>[];
}

/**
 * A NAMED limit — a row of the project's BudgetBook, referenced rather than copied. 'Carry
 * range' is a concept a project tunes, not a number retyped at each of the five sites that mention
 * it.
 */
export interface Budget extends Object {
  /**
   * What a developer called it. Surfaces in the verdict, which is how a rejection says WHICH
   * limit was missed.
   */
  readonly name: string;
  /**
   * The kind and the limit. Retuning this is the one edit that moves every site naming this
   * budget.
   */
  readonly cost: Cost;
  /**
   * Unspent amount.
   */
  remaining(): number;
  /**
   * Consume from the budget. The argument is always a DISTANCE, whatever the budget measures —
   * a caller that had to know whether to pass metres or seconds would re-derive the conversion
   * at every call site and one of them would get it wrong.
   */
  spend(x: number): void;
  /**
   * Judge a distance against what is left, naming this budget in the verdict.
   */
  judge(distance: number): InstanceRef<Verdict>;
}

/**
 * The project's named budgets — the one place 'carry range' is a number. NOTHING spends against
 * a row here: open() hands out a working copy, because spending against the shared row would make
 * two unrelated routes drain each other and the symptom would point nowhere near the cause.
 */
export interface BudgetBook extends Object {
  /**
   * Register a named budget.
   */
  declare(name: string, cost: Cost): InstanceRef<Budget>;
  /**
   * Change what it costs. Every site naming it moves at once, which is the point; a site that
   * inlined the number does not, which is also the point.
   */
  retune(budget: InstanceRef<Budget>, cost: Cost): void;
  /**
   * Look one up by the name a developer typed.
   */
  by_name(name: string): InstanceRef<Budget>;
  /**
   * A working copy to spend against. Null for a reference the book does not hold — a dangling
   * budget is a load-time diagnostic, never a default limit quietly standing in.
   */
  open(budget: InstanceRef<Budget>): InstanceRef<Budget>;
}

/**
 * Metres.
 */
export interface DistanceCost extends Cost {
  /**
   * World units.
   */
  limit: number;
}

/**
 * Seconds. Every TimeCost is a distance divided by player_profile.speed, which is why that setting
 * is not optional.
 */
export interface TimeCost extends Cost {
  /**
   * Seconds.
   */
  limit: number;
  /**
   * World units per second. Without it there is no way to turn seconds into a reachable
   * distance.
   */
  speed: number;
}

/**
 * Draw against a named resource pool at a rate. How a soft gate is a magnitude rather than a rule
 * — the solver can trade it off instead of treating it as impassable.
 */
export interface PoolCost extends Cost {
  /**
   * Which declared resource.
   */
  pool: string;
  /**
   * How much of the pool may be drawn.
   */
  limit: number;
  /**
   * Draw per world unit travelled.
   */
  rate: number;
}

/**
 * A row of the project's BudgetBook — retune it in one place and every site naming it moves.
 */
export interface NamedBudget extends BudgetRef {
  /**
   * Which named budget.
   */
  budget: InstanceRef<Budget>;
}

/**
 * A cost authored at this site. Right for a one-off; a magic number if it repeats — and because
 * inline and named are told apart, a tool can notice when it has.
 */
export interface InlineBudget extends BudgetRef {
  /**
   * What it costs, here.
   */
  cost: Cost;
}

/**
 * A realised route. What a Route becomes once the generator has produced it — and the reason
 * there is no spline resource.
 */
export interface Path extends Object {
  /**
   * The ordered steps.
   */
  steps(): PathStep[];
  /**
   * Total distance.
   */
  length(): number;
  /**
   * Net vertical change.
   */
  rise(): number;
  /**
   * Start point.
   */
  origin(): Vec3;
  /**
   * End point.
   */
  target(): Vec3;
}

/**
 * One leg of a path.
 */
export interface PathStep extends Object {
  /**
   * Where this leg starts.
   */
  position(): Vec3;
  /**
   * Leg distance.
   */
  length(): number;
  /**
   * What is underfoot.
   */
  surface(): ClassPath<Surface>;
  /**
   * Which Floor this leg is on.
   */
  floor(): ScopeHandle;
  /**
   * What made this leg possible.
   */
  via(): InstanceRef<Object>;
}

/**
 * What requires() returns. The channel that makes the generator place enabling content.
 */
export interface PlacementNeed extends Object {
}

/**
 * Something carrying this component must exist, accessible by this route.
 */
export interface NeedsActor extends PlacementNeed {
  /**
   * The component the needed actor must carry. A CLASS reference — nothing is constructed.
   */
  having: ClassPath<Component>;
  /**
   * How it must be accessible.
   */
  route: InstanceRef<Route>;
}

/**
 * This volume must stay empty.
 */
export interface NeedsClearance extends PlacementNeed {
  /**
   * The volume.
   */
  volume: InstanceRef<CollisionBody>;
}

/**
 * Place me ON an edge of this kind and close it.
 */
export interface BlocksTraversal extends PlacementNeed {
  /**
   * Which edges qualify.
   */
  matching: ClassPath<TraversalComponent>;
}

/**
 * A hard placement rule. Constraints express what CONTENT can state; dials express what only the
 * generator can decide.
 */
export interface Constraint extends Object {
}

/**
 * No sibling of this kind in the same scope.
 */
export interface AloneInScope extends Constraint {
  /**
   * Which scope must be exclusive.
   */
  scope: InstanceScope;
}

/**
 * At least this far from a named kind. A door writes key-to-lock distance here, because the door
 * names its own unlock and the key does not know its lock.
 */
export interface MinDistanceFrom extends Constraint {
  /**
   * What to stay away from.
   */
  kind: ClassPath<Object>;
  /**
   * The minimum separation.
   */
  budget: BudgetRef;
}

/**
 * At most this far from a named kind.
 */
export interface MaxDistanceFrom extends Constraint {
  /**
   * What to stay near.
   */
  kind: ClassPath<Object>;
  /**
   * The maximum separation.
   */
  budget: BudgetRef;
}

/**
 * Must be mounted on a matching socket.
 */
export interface MountedOn extends Constraint {
  /**
   * Which sockets qualify.
   */
  accepts: TagQuery;
}

/**
 * Must be inside this scope.
 */
export interface WithinScope extends Constraint {
  /**
   * The scope.
   */
  scope: InstanceScope;
}

/**
 * Must not be inside this scope.
 */
export interface NotWithinScope extends Constraint {
  /**
   * The scope.
   */
  scope: InstanceScope;
}

/**
 * These instances belong together. Prefer co-locating as components of ONE Actor where that
 * applies — a landmark with two sockets is one Actor, unambiguous by construction. Reach for
 * Cohort only when the members are genuinely separate placeables.
 */
export interface Cohort extends Constraint {
  /**
   * The grouped classes.
   */
  members: ClassPath<Actor>[];
  /**
   * How tightly grouped.
   */
  scope: InstanceScope;
  /**
   * Whether a partial placement is acceptable.
   * @default true
   */
  all_or_nothing: boolean;
  /**
   * Whether the order must be learnable.
   * @default false
   */
  ordered: boolean;
}

/**
 * A soft placement bias. Relaxable, and REPORTED when relaxed — nothing is loose by accident.
 */
export interface Preference extends Object {
  /**
   * How hard the solver tries.
   * @default PREFERRED
   */
  strictness: Strictness;
  /**
   * Relative pull.
   * @default 1.0
   */
  weight: number;
}

/**
 * What forbids() returns: a volume nothing may occupy, with declared escapes.
 */
export interface Exclusion extends Object {
  /**
   * The excluded volume.
   */
  volume: InstanceRef<CollisionBody>;
  /**
   * Content permitted anyway.
   */
  unless: ClassPath<Object>[];
  /**
   * Prose for the trace.
   */
  reason: string;
}

/**
 * Ordering relative to other content.
 */
export interface ScheduleRule extends Object {
  /**
   * Whether the solver may break this when infeasible — and report it.
   * @default true
   */
  relaxable: boolean;
}

/**
 * Place me after this target, with a gap.
 */
export interface PlacedAfter extends ScheduleRule {
  /**
   * What must come first.
   */
  target: ClassPath<Object>;
  /**
   * How far after, in spheres.
   */
  gap: Span;
}

/**
 * Prefer not to appear alongside this.
 */
export interface ExclusiveWith extends ScheduleRule {
  /**
   * The other content.
   */
  other: ClassPath<Object>;
  /**
   * How strongly.
   */
  weight: number;
}

/**
 * This replaces a base once available — the upgrade relationship.
 */
export interface Supersedes extends ScheduleRule {
  /**
   * What this supersedes.
   */
  base: ClassPath<Object>;
}

/**
 * Pin to a sphere range. The first constraint about PACING rather than topology, and the one
 * developers reach for soonest.
 */
export interface SpherePin extends ScheduleRule {
  /**
   * Permitted spheres.
   */
  range: Span;
}

/**
 * Why the core did what it did. The trace is built from these.
 */
export interface Rationale extends Object {
  /**
   * What this explains.
   */
  subject(): InstanceRef<Object>;
  /**
   * Which pipeline layer decided.
   */
  layer(): number;
  /**
   * What the decision read.
   */
  inputs(): InstanceRef<Object>[];
  /**
   * Prose.
   */
  explain(): string;
  /**
   * The upstream reasons, so a developer can walk back to the root cause.
   */
  because(): InstanceRef<Rationale>[];
}

/**
 * The lens handed into every hook. Scope reads are FIELDS because they are free; queries are
 * METHODS because they are not.
 *
 * Sealed: content may not subclass this.
 */
export interface Context extends Object {
  /**
   * The World scope.
   */
  readonly world: ScopeHandle;
  /**
   * The enclosing Reach. The ONLY legal use of the reach stem — everywhere else the noun is
   * range and the verb is accessible.
   */
  readonly reach: ScopeHandle;
  /**
   * The enclosing Area.
   */
  readonly area: ScopeHandle;
  /**
   * The enclosing Space, which is what bounds geometry queries.
   */
  readonly space: ScopeHandle;
  /**
   * The enclosing Floor, which is what partitions accessibility.
   */
  readonly floor: ScopeHandle;
  /**
   * The content being considered at a position.
   */
  readonly spatial: InstanceRef<Actor>;
  /**
   * The spine slot in play, or null outside a spine. Read-only.
   */
  readonly slot: InstanceRef<SpineSlot>;
  /**
   * Who is being reasoned about.
   */
  readonly occupant: Occupant;
  /**
   * Everyone travelling together.
   */
  readonly party: Occupant[];
  /**
   * Unlocks held. ROWS — one currency with grants() and HoldsRule. Already expanded through
   * supersedes, so membership is a plain set test.
   */
  readonly held: Unlock[];
  /**
   * The current accessibility sphere.
   */
  readonly sphere: number;
  /**
   * Normalised progress through the world.
   */
  readonly progression: number;
  /**
   * The role assigned to the content under consideration.
   */
  readonly role: Role;
  /**
   * Which pipeline layer is asking.
   */
  readonly layer: number;
  /**
   * How real the geometry currently is.
   */
  readonly fidelity: Fidelity;
  /**
   * The bounded error of this fidelity rung. The ladder is monotone, so this only ever shrinks.
   */
  readonly tolerance: number;
  /**
   * The ONLY randomness source. Anything else is unreplayable.
   */
  readonly rng: Rng;
  /**
   * Read a declared world-state variable.
   */
  state_of(name: string): string;
  /**
   * Read a declared resource pool.
   */
  pool(name: string): Pool;
  /**
   * Read a project setting.
   */
  setting(name: string): MetaValue;
  /**
   * The three-axis query builder: what to trace, what to consider, what to report. Declarative
   * filters, never closures — a predicate callback survives neither the binding contract nor
   * the palette.
   */
  query(): Query;
  /**
   * Realise a path to a target.
   */
  path_to(target: InstanceRef<Object>, f: QueryFilter): InstanceRef<Path>;
  /**
   * Can an occupant holding these unlocks get from here to there? Trivalent, not bool, because
   * the API must not be able to lie.
   */
  accessible(from: Vec3, to: Vec3, held: Unlock[]): Trivalent;
  /**
   * Trivalent for METRIC questions. Dual bounds answer set membership; this answers 'is this
   * ledge within 30 m', which every Span and Budget comparison actually asks.
   */
  within(measured: number, limit: number): Trivalent;
  /**
   * Ask the solver for something, softly.
   */
  request(p: InstanceRef<Preference>): void;
  /**
   * Add a reason to the trace.
   */
  note(r: InstanceRef<Rationale>): void;
  /**
   * One-way notification to the host. Observational only, never affecting generation; debug_only
   * messages are stripped entirely from a cooked build.
   */
  send_message(text: string, channel: string, debug_only: boolean): void;
}

/**
 * A handle on one scope.
 *
 * Sealed: content may not subclass this.
 */
export interface ScopeHandle extends Object {
  /**
   * Extent.
   */
  readonly bounds: Aabb;
  /**
   * Peers under the same parent.
   */
  readonly siblings: ScopeHandle[];
  /**
   * Floors inside this scope, if it is a Space.
   */
  readonly floors: ScopeHandle[];
  /**
   * Placed actors here.
   */
  readonly instances: InstanceRef<Actor>[];
  /**
   * Unlocks obtainable in this scope.
   */
  readonly granted_here: Unlock[];
  /**
   * Is this actor inside?
   */
  contains(a: InstanceRef<Actor>): boolean;
  /**
   * Can an occupant holding these get here from there?
   */
  accessible_from(other: ScopeHandle, held: Unlock[]): Trivalent;
  /**
   * Placed content of a kind. Space and up — there is deliberately no floor-scoped instance
   * query, because it would stop at a boundary the geometry does not stop at.
   */
  instances_of(kind: ClassPath<Object>, scope: InstanceScope): InstanceRef<Object>[];
  /**
   * Read a numeric dial by its qualified <ClassName>.<DialName> id -- a scope handle may be any
   * scope, so the owner is never implied. This is the DYNAMIC read; the typed one is the
   * per-dial get node, which is picked and carries the dial's real type. Inherits OUTWARD-IN and
   * an inner scope wins, so 'set saturation once at World scope' works. The trace records which
   * scope supplied the value.
   */
  dial(id: string): number;
}

/**
 * 2D vector. components, length, normalized, dot, distance_to, arithmetic, and the ZERO/ONE
 * constants.
 */
export interface Vec2 {
}

/**
 * 3D vector. components, length, normalized, dot, cross, distance_to, arithmetic, and the
 * ZERO/ONE/UP/DOWN/FORWARD/RIGHT constants.
 */
export interface Vec3 {
}

/**
 * Rotation. from_euler, to_euler, slerp, mul.
 */
export interface Quaternion {
}

/**
 * 4x4 matrix. origin, basis, apply, inverse, mul.
 */
export interface Mat4 {
}

/**
 * Position, rotation and scale. origin, basis, apply, inverse, mul.
 */
export interface Transform {
}

/**
 * Axis-aligned box. min, max, center, size, contains, intersects, expand, merge.
 */
export interface Aabb {
}

/**
 * Origin, direction, at(t).
 */
export interface Ray {
}

/**
 * Normal, d, distance_to(p).
 */
export interface Plane {
}

/**
 * An inclusive range. min, max, contains, clamp, length, overlaps, lerp, is_bounded, and
 * UNBOUNDED. A Span DECLARES a range; a Route OBLIGES one.
 */
export interface Span {
}

/**
 * ONE ROW of an UnlockTableResource -- one atom of the progression lattice, something an occupant
 * holds or knows. id, name, doc, supersedes. NOT A CLASS AND NOT A FILE: it carries no behaviour
 * whatever, because every mechanical consequence belongs to a Component where
 * affords/supports/judge can act on it. An unlock is an identity, and identity is all it is.
 */
export interface Unlock {
}

/**
 * 2D point data and NOT a resource — one row of a CurveTableResource. points, interpolation,
 * sample, constant, ramp, from_points.
 */
export interface Curve {
}

/**
 * A DEVELOPER-AUTHORED, named, tunable value owned by a Schematic or a Spine slot — how a host
 * keeps fine-grained control over authored content at runtime. Identity is <ClassName>.<DialName>.
 * Always exposed; always optional; the core ships none, and there is no such thing as a core dial.
 * Holds a number, a hard range, a soft AdaptiveRange, an enum value, one curve, or a whole curve
 * table whose named eval input it drives for every row. Resolves ONCE per generation pass, so it
 * never changes underneath a decision mid-pass — which is why changing one is a different recipe
 * and regenerates the world.
 */
export interface Dial {
}

/**
 * soft_min, hard_max, and target(available) computed from what is genuinely placeable. Falls below
 * soft_min HONESTLY under content scarcity rather than padding with repeated filler or breaking
 * outright.
 */
export interface AdaptiveRange {
}

/**
 * initial, max, consumable.
 */
export interface Quantity {
}

/**
 * A resource an occupant draws on. capacity, reserve, and available(ctx) — the last is what
 * every soft gate compares against, because capacity alone cannot answer how much is held at a
 * given sphere.
 */
export interface Pool {
}

/**
 * radius, severity 0..1, avoidable, continuous, mitigated_by, and NONE.
 */
export interface Harm {
}

/**
 * permitted (a Rule), max_slope, endurance (a Budget). Returned per occupant by Surface.supports.
 */
export interface Support {
}

/**
 * distance (a Span), max_slope, surface. What an occupant needs at the near end of a traversal.
 */
export interface Approach {
}

/**
 * Who is standing, as a parameter. actor (null for the player), is_player, held, holds(kind),
 * footprint.
 */
export interface Occupant {
}

/**
 * A query result. valid, distance, point, normal, actor, component, island, polygon, triangle,
 * surface, face, fidelity, and certainty. Fields below the achieved detail are ABSENT, which is
 * checkable rather than sentinel-valued.
 */
export interface Hit {
}

/**
 * axis and angle. Replaces a three-value enum so that 'up, level or diagonally up, never down' is
 * expressible.
 */
export interface DirectionCone {
}

/**
 * A picked CLASS path. path, is_a, and defaults() — the core-owned class default, one per class,
 * READ and never built by content. This is how a token's authored values are compared without
 * instantiating anything.
 */
export interface Kind {
}

/**
 * A live INSTANCE reference. Never interchangeable with Kind: a class never appears in a value
 * position and a path never in a type position.
 */
export interface Ref {
}

/**
 * A dotted hierarchical label, picked rather than typed.
 */
export interface Tag {
}

/**
 * A tag match with an exact/inherited toggle. Why an eligible-surface list survives every future
 * surface being added.
 */
export interface TagQuery {
}

/**
 * A kind and a limit, with NO accounting — Distance(m), Time(s) or Pool(pool, rate). What a
 * Budget is a named instance of, and what an interaction spends.
 *
 * Sealed: content may not subclass this.
 */
export interface Cost {
}

/**
 * The forms of Cost. Switch on `form` — TypeScript narrows each arm.
 */
export type CostForm =
  | (DistanceCost & { form: "DistanceCost" })
  | (TimeCost & { form: "TimeCost" })
  | (PoolCost & { form: "PoolCost" });

/**
 * 'This budget' — Named(Ref<Budget>) into the project's book, or Inline(Cost) authored at the
 * site. BOTH forms stay and stay DISTINGUISHABLE: forcing a one-off through the book is ceremony
 * for a number used once, and because the two are told apart a tool can notice the same inline
 * number in twelve places and offer to extract it.
 *
 * Sealed: content may not subclass this.
 */
export interface BudgetRef {
}

/**
 * The forms of BudgetRef. Switch on `form` — TypeScript narrows each arm.
 */
export type BudgetRefForm =
  | (NamedBudget & { form: "NamedBudget" })
  | (InlineBudget & { form: "InlineBudget" });

/**
 * How many of something may exist, per scope.
 */
export interface Quota {
}

/**
 * An editor-time finding. Never blocks generation.
 */
export interface Diagnostic {
}

/**
 * The owned, forkable PRNG. Reached only through ctx.rng.
 */
export interface Rng {
}

/** Which face of a bounding box something presents or mounts against. */
export type Face =
  | "POS_X"
  | "NEG_X"
  /** Up. The default mount face — a floor-standing thing. */
  | "POS_Y"
  /** Down. A ceiling mount. */
  | "NEG_Y"
  | "POS_Z"
  | "NEG_Z";

/** What a placement turned out to be. An OUTPUT, assigned after the search from what it ran into — never declared. */
export type Role =
  /** No mechanical consequence. */
  | "DECORATION"
  | "OBSTACLE"
  | "TRAVERSAL"
  | "GATE"
  | "LANDMARK";

/** What kind of reward something is. An INPUT the developer declares. */
export type ItemClass =
  /** May appear in logic. The conservative default. */
  | "PROGRESSION"
  /** Tunes route difficulty, spends slack, never gates. */
  | "USEFUL"
  /** Rewards optional exploration. Auto-assigned to anything accessible solely through a relaxation. */
  | "BONUS"
  /** Satisfies density. Currency, ammo, consumables. */
  | "FILLER";

/** Which collision layer a body belongs to. */
export type CollisionLayer =
  | "HULL"
  | "STATIC"
  | "DYNAMIC";

/** Coarse query scoping. */
export type QueryFilter =
  | "ALL"
  | "NONE"
  | "WORLD"
  | "PLACED";

/** PROPOSED. How collision is derived from geometry. The design names the enum but not its values; these are the minimum set the pipeline needs and must be confirmed when mesh import lands. */
export type CollisionMode =
  /** Visible, never collidable. */
  | "NONE"
  | "CONVEX_HULL"
  | "DECOMPOSED"
  | "TRIANGLE_MESH"
  /** Use a separately authored collision body rather than deriving one. */
  | "AUTHORED";

/** Whether and how a supply comes back. */
export type Replenish =
  | "NEVER"
  | "ON_REENTER"
  | "FROM_SOURCE"
  | "ON_TIMER";

/** How wide an instance query or effect reaches. There is deliberately NO FLOOR member: a floor-scoped instance query would stop at a boundary the geometry does not stop at. */
export type InstanceScope =
  | "SPATIAL"
  | "SPACE"
  | "AREA"
  | "REACH"
  | "WORLD";

/** Constructive solid geometry operations. */
export type BooleanOp =
  | "UNION"
  | "SUBTRACT"
  | "INTERSECT";

/** How committed a node is. A subtree may lag but never lead. */
export type ResolveState =
  /** A revisable forecast. The only state from which a node may be removed. */
  | "PROJECTED"
  /** Committed to exist, with an envelope claimed. Removable only by recorded backtracking. */
  | "RESERVED"
  /** Built and frozen. */
  | "REALIZED";

/** How much a query wants back. Detail is what you ASK for; fidelity is what EXISTS. */
export type Detail =
  /** Which room. Available from L1. */
  | "SCOPE"
  /** Which box, which face. From L2. */
  | "COLLIDER"
  /** Which placed actor, with its content path and metadata. From L2. */
  | "INSTANCE"
  /** Which mesh island. From L3. */
  | "ISLAND"
  /** Which hull polygon or occupancy cell. From L3. */
  | "POLYGON"
  /** Which triangle, with an interpolated normal. From L4. */
  | "TRIANGLE";

/** How real the geometry is. The ladder is monotone — each rung only tightens, so tolerance only shrinks. */
export type Fidelity =
  /** Tolerance is the envelope's own slack. */
  | "ENVELOPE"
  /** Tolerance is the contouring tolerance. */
  | "HULL"
  /** Tolerance is zero. */
  | "GEOMETRY";

/** How hard a spine slot or preference must hold. Every relaxation is declared; nothing is loose by accident. */
export type Strictness =
  /** Must hold; if it cannot, generation fails with a diagnostic. */
  | "REQUIRED"
  /** Strongly biased; may be relaxed when infeasible, and is REPORTED when it is. */
  | "PREFERRED"
  /** The generator decides freely; absence is expected. */
  | "OPTIONAL";

/** Per-lock sequence-break policy. A designer marks two or three gates, not two hundred. */
export type SkipPolicy =
  /** A path exists; other emergent paths are fine. The default, because real games ship tolerated skips deliberately. */
  | "TOLERATED"
  /** Report every alternative route found. */
  | "EXACT"
  /** Actively verify no alternative exists at that sphere, and fail loudly if one does. Also what decides whether a discovered shortcut may be adopted. */
  | "GUARDED";

/** How a curve row interpolates between keys. Declared PER ROW — UE fixes it per table only because a CSV has one header row, which JSON does not. */
export type Interpolation =
  | "CONSTANT"
  | "LINEAR"
  | "CUBIC";

/** Three-valued truth. A confident wrong answer is worse than an admitted unknown, so the API returns this rather than a bool wherever geometry is still approximate. */
export type Trivalent =
  | "YES"
  | "NO"
  /** Inside the ambiguous band. Resolve, never guess — and a decision that returns this re-asks at the next fidelity rung by construction. */
  | "AMBIGUOUS";

