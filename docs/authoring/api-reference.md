<!--
 GENERATED — do not edit.

 Source: manifest/tier1.toml
 Regenerate: cargo xtask generate

 Every edit here is lost on the next run. Change the manifest instead — it is the
 only file in this system that is authored by hand.
-->

# API reference

The tier-1 surface: **150 declarations** and **344 members**, generated from the manifest.

Notation: a **field** is a plain read and appears in a graph as a pure *get* node. A **method** takes an argument, computes, or mutates, and appears as a *call* node with execution pins — so the shape of a node tells you whether it costs anything. A **hook** is a question the core asks; hooks are what a schematic's `OVERRIDES` list is built from.

Paths are mount-pointed: `/Core/…` is tier-1.

---

## Objects

### `Object`

The root of everything with identity. Has metadata, has a rationale, can be asked what it is.

`/Core/Object`

| Field | Type | | |
|---|---|---|---|
| `id` | `ObjectId` | final | Opaque identity. A string or opaque handle — NEVER a u64, because TS integers alias past 2^53. |
| `type_path` | `String` | final | The mount-pointed class path this object was instantiated from. |
| `rationale` | `Ref<Rationale>` | final | Why the core did what it did with this object. Core-written. |
| `meta_keys` | `Array<String>` | final | Every metadata key currently set. |

| Method | Returns | | |
|---|---|---|---|
| `is_a(kind: Kind<Object>)` | `bool` | final | Class-membership test against a picked class path. |
| `equals(other: Ref<Object>)` | `bool` | final | Identity comparison. Never a float comparison. |
| `format()` | `String` |  | Short display form. *Default: TypeName#id.* |
| `explain()` | `String` |  | Prose a developer reads in the trace. *Default: format().* |
| `meta(key: String)` | `MetaValue` | final | Read one metadata value. |
| `set_meta(key: String, value: MetaValue)` | `void` | final | Write one metadata value. The CV_ prefix is reserved for the core and rejected here. |
| `has_meta(key: String)` | `bool` | final | Is this key set? |
| `remove_meta(key: String)` | `void` | final | Clear one metadata value. |
| `_configure()` | `void` | **hook** | Setup phase 1. Own configuration only; other content may not exist yet. |
| `_resolve()` | `void` | **hook** | Setup phase 2. Every registered content class now exists and may be referenced. |

### `Actor` — extends `Object`

Something placeable in a world. The only thing the core places, and the sole point of contact between content and generation.

`/Core/Actor`

| Field | Type | | |
|---|---|---|---|
| `components` | `Array<Ref<Component>>` | final | Attached components, in attach order — which is the order every aggregating hook walks. |
| `parent` | `Ref<Actor>` | final | The actor this one is mounted on, if any. |
| `children` | `Array<Ref<Actor>>` | final | Actors mounted on this one. |
| `transform` | `Transform` | mutable · exposed · final | Local transform relative to the parent. |
| `skip_policy` | `SkipPolicy` | mutable · exposed | Whether alternative routes past this actor's gate are tolerated, reported, or actively forbidden. Inert unless gate() is non-trivial. *Default: TOLERATED.* |
| `discoverability` | `float` | mutable · exposed | The declared likelihood a player finds this gate's solution unprompted. Declared because it is not derivable; above the threshold the solver treats the gate as open, conservatively, and names it in the trace. *Default: 1.0.* |

| Method | Returns | | |
|---|---|---|---|
| `component(kind: Kind<Component>)` | `Ref<Component>` | final | First attached component of that kind. |
| `components_of(kind: Kind<Component>)` | `Array<Ref<Component>>` | final | Every attached component of that kind. |
| `add_component(c: Ref<Component>)` | `void` | final | Attach a component. |
| `remove_component(c: Ref<Component>)` | `void` | final | Detach a component. |
| `attach_to(parent: Ref<Actor>, mount: Ref<MountComponent>)` | `void` | final | Mount this actor onto another at a named socket. |
| `world_transform()` | `Transform` | final | Absolute transform. Walks the parent chain, so it is not free. |
| `pivot(ctx: Ref<Context>)` | `Vec3` | **hook** | The point this actor rotates and mounts about. *Default: Vec3.ZERO.* |
| `bounds(ctx: Ref<Context>)` | `Aabb` | **hook** | Axis-aligned extent. *Default: union of component bounds.* |
| `footprint(ctx: Ref<Context>)` | `Ref<CollisionBody>` | **hook** | The space this reserves during skeleton layout. *Default: union of component collision.* |
| `clearance(ctx: Ref<Context>)` | `Ref<CollisionBody>` | **hook** | Space that must stay empty around this actor. Called per candidate position, so it must stay cheap. *Default: empty.* |
| `collision(ctx: Ref<Context>)` | `Ref<CollisionBody>` | **hook** | What this actor physically blocks. *Default: union of component collision.* |
| `mount_faces(ctx: Ref<Context>)` | `Array<Face>` | **hook** | Which faces this may be mounted against. Floor, wall or ceiling is what distinguishes a chest from a sconce from a chandelier. *Default: [ POS_Y ].* |
| `up_axis(ctx: Ref<Context>)` | `Vec3` | **hook** | Which direction is up for this actor. *Default: Vec3.UP.* |
| `yaw_snap(ctx: Ref<Context>)` | `int` | **hook** | Rotational quantisation about the up axis. 0 = free, 4 = 90 degrees. *Default: 4.* |
| `allow_pitch(ctx: Ref<Context>)` | `bool` | **hook** | May the solver tilt this out of the up axis? *Default: false.* |
| `allow_roll(ctx: Ref<Context>)` | `bool` | **hook** | May the solver roll this about its forward axis? *Default: false.* |
| `scalable(ctx: Ref<Context>)` | `Span` | **hook** | Permitted uniform scale range. A lever a fixed-template system cannot have; convexity is affine-invariant, so the collision cache survives it. *Default: Span(1, 1).* |
| `uniform_scale_only(ctx: Ref<Context>)` | `bool` | **hook** | Forbid non-uniform scaling. *Default: true.* |
| `embed_depth(ctx: Ref<Context>)` | `Span` | **hook** | How far this may sink into the surface it mounts on. *Default: Span(0, 0).* |
| `constraints(ctx: Ref<Context>)` | `Array<Ref<Constraint>>` | **hook** | Hard placement constraints. A door names its own unlock here, which is where key-to-lock distance is written. *Default: [].* |
| `preferences(ctx: Ref<Context>)` | `Array<Ref<Preference>>` | **hook** | Soft placement biases. Relaxable, and reported when relaxed. *Default: [].* |
| `eligible_roles(ctx: Ref<Context>)` | `Array<Role>` | **hook** | Which roles this actor may be assigned. Role is an output; this only narrows the candidate set. *Default: all roles.* |
| `schedule(ctx: Ref<Context>)` | `Array<Ref<ScheduleRule>>` | **hook** | Ordering rules relative to other content. *Default: [].* |
| `quota(ctx: Ref<Context>)` | `Quota` | **hook** | How many of this may exist, per scope. |
| `weight(ctx: Ref<Context>)` | `float` | **hook** | Selection bias among eligible candidates. *Default: 1.0.* |
| `repulsion(ctx: Ref<Context>)` | `float` | **hook** | How strongly instances of this push away from each other. A pressure, never an obligation. *Default: 0.0.* |
| `enables(ctx: Ref<Context>)` | `Array<Ref<Interaction>>` | **hook** | What can be done here. *Default: aggregate enabled components in attach order.* |
| `requires(ctx: Ref<Context>)` | `Array<Ref<PlacementNeed>>` | **hook** | What must exist near me for my mechanic to work. This is the hook that makes the generator place enabling content. *Default: aggregate enabled components in attach order.* |
| `forbids(ctx: Ref<Context>)` | `Array<Ref<Exclusion>>` | **hook** | What must NOT be near me. The negative half of an obligation. *Default: aggregate enabled components in attach order.* |
| `judge(ctx: Ref<Context>, path: Ref<Path>)` | `Ref<Verdict>` | **hook** | Is this proposal good, and how wrong is it? The magnitude is the point: a boolean turns the placement search into a reroll. *Default: AcceptedVerdict.* |
| `gate(ctx: Ref<Context>)` | `Ref<Rule>` | **hook** | What the occupant must hold to pass. *Default: AlwaysRule.* |
| `harm(ctx: Ref<Context>)` | `Harm` | **hook** | Is this dangerous, how much, and is it avoidable? *Default: Harm.NONE.* |
| `grants(ctx: Ref<Context>)` | `Array<Unlock>` | **hook** | Unlocks the occupant keeps after reaching this. ROWS of an UnlockTableResource, never classes — the lattice is over identities and a row id already is one. An unlock carries NO behaviour: every mechanical consequence belongs to a Component. *Default: aggregate enabled components in attach order.* |
| `on_proposed(ctx: Ref<Context>)` | `void` | **hook** | Event: the solver is considering this position. |
| `on_placed(ctx: Ref<Context>)` | `void` | **hook** | Event: this actor was committed to a position. |
| `on_rejected(ctx: Ref<Context>, why: Ref<Verdict>)` | `void` | **hook** | Event: a candidate position was refused, with the verdict that refused it. |
| `on_obtained(ctx: Ref<Context>)` | `void` | **hook** | Event: an occupant reached this and took what it grants. |
| `on_component_changed(c: Ref<Component>)` | `void` | **hook** | Event: an attached component invalidated itself. |
| `on_finalized(ctx: Ref<Context>)` | `void` | **hook** | Event: generation is complete and this actor is realized. |
| `reset(ctx: Ref<Context>)` | `void` | **hook** | Event: the solver is backtracking past this actor. Lets a rewind be clean. |

### `Item` — extends `Actor`

An obtainable actor. The thing that hands out unlocks, as distinct from the unlocks themselves.

`/Core/Item`

| Method | Returns | | |
|---|---|---|---|
| `classification(ctx: Ref<Context>)` | `ItemClass` | **hook** | What kind of reward this is. An INPUT the developer declares, never an output — and it defaults to the conservative answer. *Default: PROGRESSION.* |
| `quantity(ctx: Ref<Context>)` | `Quantity` | **hook** | How many, and whether taking it consumes it. *Default: Quantity(1, 1, false).* |
| `replenishes(ctx: Ref<Context>)` | `Replenish` | **hook** | Whether and how this comes back. *Default: NEVER.* |

### `Component` — extends `Object`

An attachable behaviour with its own transform. The seven hooks have the same signatures as Actor, and this is where a mechanic is written.

`/Core/Component`

| Field | Type | | |
|---|---|---|---|
| `owner` | `Ref<Actor>` | final | The actor this is attached to. |
| `enabled` | `bool` | mutable · exposed | Disabled components are skipped by every aggregating hook. *Default: true.* |
| `transform` | `Transform` | mutable · exposed · final | Local transform relative to the owner. *Default: identity.* |

| Method | Returns | | |
|---|---|---|---|
| `world_transform()` | `Transform` | final | Absolute transform. Walks the parent chain, so it is not free. |
| `invalidate()` | `void` | final | Signal that this component changed; the owner receives on_component_changed. |
| `on_attached(owner: Ref<Actor>)` | `void` | **hook** | Event: attached to an actor. |
| `on_detached()` | `void` | **hook** | Event: detached from an actor. |
| `validate()` | `Array<Diagnostic>` | **hook** | Editor-time self-check. Never blocks generation. *Default: [].* |
| `replenishes(ctx: Ref<Context>)` | `Replenish` | **hook** | Whether this component's supply comes back. Not only Items: a dispenser and an environmental source are not Items. *Default: NEVER.* |

### `MeshComponent` — extends `Component`

Imported geometry, and the material-name to Surface mapping that gives it meaning.

`/Core/MeshComponent`

| Field | Type | | |
|---|---|---|---|
| `asset` | `Resource<MeshResource>` | mutable · exposed | The mesh file this draws and derives collision from. |
| `surfaces` | `Map<String, Kind<Surface>>` | mutable · exposed | Submesh or material name to Surface class. This is how imported art acquires generation meaning. |
| `collision_mode` | `CollisionMode` | mutable · exposed | How collision is derived from the mesh. |
| `visible` | `bool` | mutable · exposed | Visible is not the same as collidable. *Default: true.* |

| Method | Returns | | |
|---|---|---|---|
| `collision()` | `Ref<CollisionBody>` | final | The derived collision body. |

### `ShapeComponent` — extends `Component`

A parametric shape. Collision is computed analytically from its parameters, never from tessellation.

`/Core/ShapeComponent`

| Field | Type | | |
|---|---|---|---|
| `shape` | `Shape` | mutable · exposed | The parametric primitive. |
| `surface` | `Kind<Surface>` | mutable · exposed | The Surface class this shape presents. |
| `collision_mode` | `CollisionMode` | mutable · exposed | How collision is derived. |
| `visible` | `bool` | mutable · exposed | Visible is not the same as collidable. *Default: true.* |

| Method | Returns | | |
|---|---|---|---|
| `collision()` | `Ref<CollisionBody>` | final | The analytically derived collision body. |

### `MountComponent` — extends `Component`

An attachment socket. What may mount here is a tag query, not a hand-maintained class list.

`/Core/MountComponent`

| Field | Type | | |
|---|---|---|---|
| `name` | `String` | mutable · exposed | A label for a developer reading the viewport. Deliberately not a lookup key. |
| `accepts` | `TagQuery` | mutable · exposed | Which content may mount here. |
| `faces` | `Array<Face>` | mutable · exposed | Which faces of the mounted thing may meet this socket. |
| `clearance` | `Ref<CollisionBody>` | mutable · exposed | Space that must stay empty around whatever mounts here. |

| Method | Returns | | |
|---|---|---|---|
| `admits(ctx: Ref<Context>, candidate: Ref<Actor>)` | `Ref<Rule>` | **hook** | Whether a specific candidate may mount, beyond the tag query. *Default: AlwaysRule.* |

### `TraversalComponent` — extends `Component`

Turns a spatial delta into a directed graph edge. The thing that makes a staircase a route rather than scenery.

`/Core/TraversalComponent`

| Method | Returns | | |
|---|---|---|---|
| `run(ctx: Ref<Context>)` | `Span` | **hook** | Horizontal distance this move covers. |
| `rise(ctx: Ref<Context>)` | `Span` | **hook** | Vertical delta this move covers. Negative is downward. |
| `direction(ctx: Ref<Context>)` | `DirectionCone` | **hook** | Permitted directions as a cone, so 'up, level or diagonally up, never down' is expressible. |
| `admits(ctx: Ref<Context>, occupant: Occupant, dir: Vec3)` | `Ref<Rule>` | **hook** | What the occupant must hold to make this move, IN THIS DIRECTION — so one edge can be open one way and gated the other. *Default: AlwaysRule.* |
| `cost(ctx: Ref<Context>)` | `float` | **hook** | What traversing this spends against a route budget. *Default: 1.0.* |
| `approach(ctx: Ref<Context>)` | `Approach` | **hook** | What the occupant needs at the near end before the move is possible. |
| `clearance(ctx: Ref<Context>)` | `Ref<CollisionBody>` | **hook** · ▶ proposed | PROPOSED. The swept volume that must be empty for the move to exist. run and rise describe only the endpoints, and admit a jump the real arc would not clear under a low ceiling. *Default: the box implied by run x rise.* |

### `CheckpointComponent` — extends `Component`

A place that returns the world to a known-good state. P15's second satisfaction route, and what lets the solver take an attractive one-way transition instead of refusing every irreversible edge.

`/Core/CheckpointComponent`

| Field | Type | | |
|---|---|---|---|
| `restores` | `Array<Kind<Object>>` | mutable · exposed | Which CLASSES OF PLACED CONTENT respawn here -- consumables, destructibles, enemies. NOT unlocks: an unlock is monotone and can never be lost, so restoring one has no meaning. |
| `restores_occupant` | `bool` | mutable · exposed | Whether the occupant also returns here — which is what makes this a respawn point as well. *Default: false.* |
| `scope` | `InstanceScope` | mutable · exposed | How far this checkpoint's effect reaches. |

### `FastTravelComponent` — extends `Component`

A node in a travel network. Not cosmetic: a network collapses traversal cost across the whole World, and difficulty here IS slack spent against a budget.

`/Core/FastTravelComponent`

| Field | Type | | |
|---|---|---|---|
| `network` | `String` | mutable · exposed | Nodes sharing a network name connect to each other. |
| `cost` | `BudgetRef` | mutable · exposed | What one hop spends — in the same currency as the routes it competes with. |
| `unlocked_by` | `Ref<Rule>` | mutable · exposed | What the occupant must hold before this node joins its network. *Default: AlwaysRule.* |

### `StateSetterComponent` — extends `Component`

Writes a world-state variable. The authoring surface for the non-monotone axis.

`/Core/StateSetterComponent`

| Field | Type | | |
|---|---|---|---|
| `variable` | `String` | mutable · exposed | Which declared state variable this writes. |
| `to_value` | `String` | mutable · exposed | The value it writes. |
| `while_occupied_by` | `Kind<Component>` | mutable · exposed | If set, the value holds only WHILE something carrying this component is present — which is what makes dwell an ordinary reading rather than a trigger kind. |

### `BlocksTraversalComponent` — extends `Component`

Place me on an edge of this kind, and close it. Without this a barrier can only be authored as geometry, which violates gate-never-delete by construction.

`/Core/BlocksTraversalComponent`

| Field | Type | | |
|---|---|---|---|
| `matching` | `Kind<TraversalComponent>` | mutable · exposed | Which kind of traversal edge this may be placed on. |
| `route` | `Ref<Route>` | mutable · exposed | The route this blocker obliges or forbids. |

### `Surface` — extends `Object`

What a piece of geometry means to a mechanic. Answers who may be here and what works here.

`/Core/Surface`

| Field | Type | | |
|---|---|---|---|
| `name` | `String` | mutable · exposed | Display name. |
| `tags` | `Array<Tag>` | mutable · exposed | Why an eligible-surface list is a TagQuery rather than a hand-maintained class list. The editor must show these on selected geometry. |

| Method | Returns | | |
|---|---|---|---|
| `supports(ctx: Ref<Context>, occupant: Occupant)` | `Array<Support>` | **hook** | Can something BE here? A state you are in, evaluated per occupant and per sphere. *Default: [ Support(AlwaysRule) ].* |
| `affords(ctx: Ref<Context>, attempt: Ref<Interaction>)` | `Ref<Rule>` | **hook** | Does an ACTION work here? Covers transit THROUGH as well as action UPON, which is how one pane of glass blocks ballistics and walking while passing a laser and a sightline. *Default: AlwaysRule.* |
| `friction(ctx: Ref<Context>)` | `float` | **hook** | Traversal resistance. *Default: 1.0.* |
| `harm(ctx: Ref<Context>)` | `Harm` | **hook** | Is standing here dangerous? *Default: Harm.NONE.* |
| `admits_mount(ctx: Ref<Context>, m: Ref<MountComponent>)` | `Ref<Rule>` | **hook** | May something mount against this surface? *Default: AlwaysRule.* |

### `Rule` — extends `Object`

**sealed** — content may not subclass this

Sealed on purpose: the solver walks this tree as the analysable half of a gate, so a developer-defined rule would make the dependency walk impossible.

`/Core/Rule`

| Method | Returns | | |
|---|---|---|---|
| `is_open()` | `bool` | final | Does this rule currently hold? |
| `referenced()` | `Array<Unlock>` | final | Every unlock this rule mentions. The solver's dependency walk, and what lets requires() plant a source. |
| `explain()` | `String` |  | Prose for a lock badge and the trace. |

### `AlwaysRule` — extends `Rule`

**sealed** — content may not subclass this

Trivially satisfied. The identity of AllOfRule.

`/Core/AlwaysRule`

### `NeverRule` — extends `Rule`

**sealed** — content may not subclass this

Never satisfied. The identity of AnyOfRule. On a sole route this is an authoring error and is reported with the numbers, not silently accepted.

`/Core/NeverRule`

### `HoldsRule` — extends `Rule`

**sealed** — content may not subclass this

The occupant holds this unlock, or anything that supersedes it.

`/Core/HoldsRule`

| Field | Type | | |
|---|---|---|---|
| `unlock` | `Unlock` | mutable · exposed | The unlock required. Satisfied by this row, or by any held unlock whose supersedes closure contains it. |
| `count` | `int` | mutable · exposed | How many. Quantity lives here, not in a number of instances. *Default: 1.* |

### `HasComponentRule` — extends `Rule`

**sealed** — content may not subclass this

The occupant holds something CARRYING a matching component. A DIFFERENT question from HoldsRule: this asks about the things held, that asks about the lattice.

`/Core/HasComponentRule`

| Field | Type | | |
|---|---|---|---|
| `kind` | `Kind<Component>` | mutable · exposed | The component class required. |

### `AllOfRule` — extends `Rule`

**sealed** — content may not subclass this

Every child holds.

`/Core/AllOfRule`

| Field | Type | | |
|---|---|---|---|
| `children` | `Array<Ref<Rule>>` | mutable · exposed | The conjuncts. |

### `AnyOfRule` — extends `Rule`

**sealed** — content may not subclass this

At least one child holds. This is what proves genuine alternate routes rather than one route the solver happened to pick.

`/Core/AnyOfRule`

| Field | Type | | |
|---|---|---|---|
| `children` | `Array<Ref<Rule>>` | mutable · exposed | The disjuncts. |

### `NegateRule` — extends `Rule`

**sealed** — content may not subclass this

The inner rule does NOT hold. Non-monotonic, so it is accepted and reported rather than refused.

`/Core/NegateRule`

| Field | Type | | |
|---|---|---|---|
| `inner` | `Ref<Rule>` | mutable · exposed | The negated rule. |

### `NearbyRule` — extends `Rule`

**sealed** — content may not subclass this

Something matching is accessible within range of this gate. The contextual half: a gate may depend on the WORLD rather than only the occupant, and because the dependency walk sees it, requires() can plant the source.

`/Core/NearbyRule`

| Field | Type | | |
|---|---|---|---|
| `kind` | `Kind<Object>` | mutable · exposed | What must be nearby. |
| `within` | `BudgetRef` | mutable · exposed | How near, in whatever currency the budget names. |
| `scope` | `InstanceScope` | mutable · exposed | The scope searched. |

### `Verdict` — extends `Object`

**sealed** — content may not subclass this

What judge() returns. A boolean would tell the solver nothing and turn the placement search into a reroll.

`/Core/Verdict`

### `AcceptedVerdict` — extends `Verdict`

**sealed** — content may not subclass this

It fits. The slack says how comfortably, which feeds difficulty.

`/Core/AcceptedVerdict`

| Field | Type | | |
|---|---|---|---|
| `slack` | `float` | mutable · exposed | Budget remaining after this placement. |

### `OverBudgetVerdict` — extends `Verdict`

**sealed** — content may not subclass this

Too far. Move the target closer by roughly this much — a direction the solver can act on.

`/Core/OverBudgetVerdict`

| Field | Type | | |
|---|---|---|---|
| `excess` | `float` | mutable · exposed | How much over budget. |
| `against` | `Ref<Budget>` | mutable · exposed | Which budget it was measured against. 'Over budget by 6.2' leaves a developer guessing; 'over budget by 6.2 against grapple reach' names the lever to pull. Null only where the comparison was against a bare number. |

### `BlockedVerdict` — extends `Verdict`

**sealed** — content may not subclass this

Something is in the way. Remove or reposition that, or route around it.

`/Core/BlockedVerdict`

| Field | Type | | |
|---|---|---|---|
| `by` | `Ref<Object>` | mutable · exposed | What blocked it. |

### `UnsuitableVerdict` — extends `Verdict`

**sealed** — content may not subclass this

Wrong kind of thing. Do not retry this candidate.

`/Core/UnsuitableVerdict`

| Field | Type | | |
|---|---|---|---|
| `reason` | `String` | mutable · exposed | Prose for the trace. |

### `Interaction` — extends `Object`

Something an occupant attempts. Subclassed by what relocates, which is what lets one surface answer four mechanics differently.

`/Core/Interaction`

| Field | Type | | |
|---|---|---|---|
| `range` | `Span` | mutable · exposed | How far this reaches. NOT named 'reach' — Reach is a scope. |
| `line_of_sight` | `bool` | mutable · exposed | Whether an unobstructed line is required. *Default: false.* |
| `from_standing` | `bool` | mutable · exposed | Whether the occupant must be supported to attempt this. *Default: true.* |

| Method | Returns | | |
|---|---|---|---|
| `consumes()` | `Array<Cost>` | **hook** | What performing this spends. What consumes a resource is the consumer, not the consumable. *Default: [].* |
| `gate()` | `Ref<Rule>` | **hook** | What the occupant must hold to perform this. Deliberately the same name and meaning as Actor.gate, and deliberately NOT 'requires', which on an Actor means something unrelated. *Default: AlwaysRule.* |
| `explain()` | `String` |  | Prose for the trace. |

### `Movement` — extends `Interaction`

THE PLAYER relocates, so this is a directed graph edge.

`/Core/Movement`

| Field | Type | | |
|---|---|---|---|
| `rise` | `Span` | mutable · exposed | Vertical delta. |
| `direction` | `DirectionCone` | mutable · exposed | Permitted directions. |

### `Displace` — extends `Interaction`

AN OBJECT relocates, not the player. Push, carry and throw are authored subclasses of this.

`/Core/Displace`

| Field | Type | | |
|---|---|---|---|
| `subject` | `Ref<Object>` | mutable · exposed | What moves. |

### `RemoteUse` — extends `Interaction`

NOTHING relocates — it acts at range. Sightlines, beams and ballistics are authored subclasses, which is how transit through a surface is expressed without a core enum.

`/Core/RemoteUse`

| Field | Type | | |
|---|---|---|---|
| `target` | `Kind<Object>` | mutable · exposed | What it acts upon. |

### `CollisionBody` — extends `Object`

A set of collision islands. What actually blocks, as distinct from what is drawn.

`/Core/CollisionBody`

| Method | Returns | | |
|---|---|---|---|
| `islands()` | `Array<CollisionData>` | final | The disjoint pieces. |
| `add(d: CollisionData)` | `void` | final | Add an island. |
| `remove(d: CollisionData)` | `void` | final | Remove an island. |
| `bounds()` | `Aabb` | final | Combined extent. |
| `is_convex()` | `bool` | final | Whether the whole body is convex. |
| `fit_error()` | `float` | final | How far this body diverges from the source geometry. Heat-mapped in the editor to catch a ramp collider left on a spiral staircase. |

### `Resource` — extends `Object`

Bytes on disk, content-hashed. Referenced from a schematic as a class plus an asset path, never inlined.

`/Core/Resource`

| Field | Type | | |
|---|---|---|---|
| `path` | `String` |  | The asset path. |
| `hash` | `String` |  | Content hash. Part of the fingerprint, which is what makes an asset swap change the recipe. |
| `is_loaded` | `bool` |  | Whether the bytes are resident. A resource may be REFERENCED during the solve without being loaded. |

### `MeshResource` — extends `Resource`

Imported geometry. In a cooked build this is a hash and a bounds with no triangles behind it.

`/Core/MeshResource`

| Field | Type | | |
|---|---|---|---|
| `bounds` | `Aabb` |  | Extent, available even when cooked. |
| `triangle_count` | `int` |  | Triangle count. |
| `submesh_names` | `Array<String>` |  | Material or submesh names, which MeshComponent maps to Surfaces. |

| Method | Returns | | |
|---|---|---|---|
| `derive_collision(mode: CollisionMode)` | `Ref<CollisionBody>` | final | Derive collision. In a cooked build this returns the baked body or ERRORS LOUDLY — never a silent empty body, which would turn a shipping bug into a world with no collision. |
| `export(path: String)` | `void` | final | Write the mesh out. |

### `UnlockTableResource` — extends `Resource`

The project's progression vocabulary -- named rows, each one atom of the lattice. JSON, not the block notation, because it has no nodes to notate. A project may hold any number of these files anywhere under /Content; THE FILE IS THE UNIT OF SHARING, so copying it carries the vocabulary with it.

`/Core/UnlockTableResource`

| Field | Type | | |
|---|---|---|---|
| `rows` | `Array<String>` | final | The row names, in file order. |

| Method | Returns | | |
|---|---|---|---|
| `row(name: String)` | `Unlock` | final | One row by display name. Convenience for authoring; by_id is the identity lookup. |
| `by_id(id: String)` | `Unlock` | final | One row by its stable id. IDENTITY IS THE ID, never the name -- renaming a row must rewrite zero references. |

### `CurveTableResource` — extends `Resource`

One or more NAMED CURVES over one NAMED DOMAIN AXIS. One resource type where UE has four: a vector curve is three rows, a colour curve is four. JSON, not the block notation, because it has no nodes to notate.

`/Core/CurveTableResource`

| Field | Type | | |
|---|---|---|---|
| `domain` | `String` |  | The axis rows are read over. Bound BY NAME to whichever ProgressionAxis carries it. |
| `y_label` | `String` |  | Editor-only, so the vertical axis has a name. |
| `rows` | `Array<String>` |  | Named curves. |

| Method | Returns | | |
|---|---|---|---|
| `row(name: String)` | `Curve` | final | One row as a Curve struct. |
| `sample(row: String, x: float)` | `float` | final | Read a row at a domain value. |

### `ProgressionAxis` — extends `Object`

**abstract** — a subclass must answer

Supplies the x a curve row is sampled at. Built-ins cover depth, space count, unlock count and sphere; a developer subclasses it for anything else, which is the only way to say 'complexity gains weight each time a boss is placed'.

`/Core/ProgressionAxis`

| Field | Type | | |
|---|---|---|---|
| `name` | `String` |  | What a CurveTable's domain matches against. |

| Method | Returns | | |
|---|---|---|---|
| `value(ctx: Ref<Context>)` | `float` | **hook** · abstract | The current position along this axis. No default — a subclass must answer. |

### `Depth` — extends `ProgressionAxis`

Reach index divided by Reach count.

`/Core/Depth`

### `SpaceCount` — extends `ProgressionAxis`

How many Spaces exist so far.

`/Core/SpaceCount`

### `UnlockCount` — extends `ProgressionAxis`

How many progression unlocks are held.

`/Core/UnlockCount`

### `Sphere` — extends `ProgressionAxis`

The current accessibility sphere.

`/Core/Sphere`

### `Route` — extends `Object`

A REQUIRED OR FORBIDDEN path with a budget, an occupant and predicates. The sign is in the primitive: a Span DECLARES a range, a Route OBLIGES one.

`/Core/Route`

| Field | Type | | |
|---|---|---|---|
| `from` | `Ref<Object>` | mutable · exposed | Origin. Null means unbound and resolved by the solver. |
| `to` | `Ref<Object>` | mutable · exposed | Destination. Null means unbound. |
| `budget` | `BudgetRef` | mutable · exposed | What traversing it may spend. |
| `occupant` | `Occupant` | mutable · exposed | Who traverses. Null means the player. |
| `party` | `Array<Occupant>` | mutable · exposed | Every listed occupant must traverse — an escort, not a simultaneous hold. |
| `cohesion` | `float` | mutable · exposed | Maximum separation in metres. 0 = unconstrained. *Default: 0.0.* |
| `line_of_sight` | `bool` | mutable · exposed | Whether an unobstructed line is obliged. An OBLIGATION, not an observation — which is what makes L4 keep it. *Default: false.* |
| `from_standing` | `bool` | mutable · exposed | Whether the origin must be supported. *Default: true.* |
| `forbidden` | `Array<Kind<Object>>` | mutable · exposed | Content this route must not pass. The negative half. |

### `Budget` — extends `Object`

A NAMED limit — a row of the project's BudgetBook, referenced rather than copied. 'Carry range' is a concept a project tunes, not a number retyped at each of the five sites that mention it.

`/Core/Budget`

| Field | Type | | |
|---|---|---|---|
| `name` | `String` | exposed | What a developer called it. Surfaces in the verdict, which is how a rejection says WHICH limit was missed. |
| `cost` | `Cost` | exposed | The kind and the limit. Retuning this is the one edit that moves every site naming this budget. |

| Method | Returns | | |
|---|---|---|---|
| `remaining()` | `float` | final | Unspent amount. |
| `spend(x: float)` | `void` | final | Consume from the budget. The argument is always a DISTANCE, whatever the budget measures — a caller that had to know whether to pass metres or seconds would re-derive the conversion at every call site and one of them would get it wrong. |
| `judge(distance: float)` | `Ref<Verdict>` | final | Judge a distance against what is left, naming this budget in the verdict. |

### `BudgetBook` — extends `Object`

The project's named budgets — the one place 'carry range' is a number. NOTHING spends against a row here: open() hands out a working copy, because spending against the shared row would make two unrelated routes drain each other and the symptom would point nowhere near the cause.

`/Core/BudgetBook`

| Method | Returns | | |
|---|---|---|---|
| `declare(name: String, cost: Cost)` | `Ref<Budget>` | final | Register a named budget. |
| `retune(budget: Ref<Budget>, cost: Cost)` | `void` | final | Change what it costs. Every site naming it moves at once, which is the point; a site that inlined the number does not, which is also the point. |
| `by_name(name: String)` | `Ref<Budget>` | final | Look one up by the name a developer typed. |
| `open(budget: Ref<Budget>)` | `Ref<Budget>` | final | A working copy to spend against. Null for a reference the book does not hold — a dangling budget is a load-time diagnostic, never a default limit quietly standing in. |

### `Path` — extends `Object`

A realised route. What a Route becomes once the generator has produced it — and the reason there is no spline resource.

`/Core/Path`

| Method | Returns | | |
|---|---|---|---|
| `steps()` | `Array<PathStep>` | final | The ordered steps. |
| `length()` | `float` | final | Total distance. |
| `rise()` | `float` | final | Net vertical change. |
| `origin()` | `Vec3` | final | Start point. |
| `target()` | `Vec3` | final | End point. |

### `PathStep` — extends `Object`

One leg of a path.

`/Core/PathStep`

| Method | Returns | | |
|---|---|---|---|
| `position()` | `Vec3` | final | Where this leg starts. |
| `length()` | `float` | final | Leg distance. |
| `surface()` | `Kind<Surface>` | final | What is underfoot. |
| `floor()` | `ScopeHandle` | final | Which Floor this leg is on. |
| `via()` | `Ref<Object>` | final | What made this leg possible. |

### `PlacementNeed` — extends `Object`

What requires() returns. The channel that makes the generator place enabling content.

`/Core/PlacementNeed`

### `NeedsActor` — extends `PlacementNeed`

Something carrying this component must exist, accessible by this route.

`/Core/NeedsActor`

| Field | Type | | |
|---|---|---|---|
| `having` | `Kind<Component>` | mutable · exposed | The component the needed actor must carry. A CLASS reference — nothing is constructed. |
| `route` | `Ref<Route>` | mutable · exposed | How it must be accessible. |

### `NeedsClearance` — extends `PlacementNeed`

This volume must stay empty.

`/Core/NeedsClearance`

| Field | Type | | |
|---|---|---|---|
| `volume` | `Ref<CollisionBody>` | mutable · exposed | The volume. |

### `BlocksTraversal` — extends `PlacementNeed`

Place me ON an edge of this kind and close it.

`/Core/BlocksTraversal`

| Field | Type | | |
|---|---|---|---|
| `matching` | `Kind<TraversalComponent>` | mutable · exposed | Which edges qualify. |

### `Constraint` — extends `Object`

A hard placement rule. Constraints express what CONTENT can state; dials express what only the generator can decide.

`/Core/Constraint`

### `AloneInScope` — extends `Constraint`

No sibling of this kind in the same scope.

`/Core/AloneInScope`

| Field | Type | | |
|---|---|---|---|
| `scope` | `InstanceScope` | mutable · exposed | Which scope must be exclusive. |

### `MinDistanceFrom` — extends `Constraint`

At least this far from a named kind. A door writes key-to-lock distance here, because the door names its own unlock and the key does not know its lock.

`/Core/MinDistanceFrom`

| Field | Type | | |
|---|---|---|---|
| `kind` | `Kind<Object>` | mutable · exposed | What to stay away from. |
| `budget` | `BudgetRef` | mutable · exposed | The minimum separation. |

### `MaxDistanceFrom` — extends `Constraint`

At most this far from a named kind.

`/Core/MaxDistanceFrom`

| Field | Type | | |
|---|---|---|---|
| `kind` | `Kind<Object>` | mutable · exposed | What to stay near. |
| `budget` | `BudgetRef` | mutable · exposed | The maximum separation. |

### `MountedOn` — extends `Constraint`

Must be mounted on a matching socket.

`/Core/MountedOn`

| Field | Type | | |
|---|---|---|---|
| `accepts` | `TagQuery` | mutable · exposed | Which sockets qualify. |

### `WithinScope` — extends `Constraint`

Must be inside this scope.

`/Core/WithinScope`

| Field | Type | | |
|---|---|---|---|
| `scope` | `InstanceScope` | mutable · exposed | The scope. |

### `NotWithinScope` — extends `Constraint`

Must not be inside this scope.

`/Core/NotWithinScope`

| Field | Type | | |
|---|---|---|---|
| `scope` | `InstanceScope` | mutable · exposed | The scope. |

### `Cohort` — extends `Constraint`

These instances belong together. Prefer co-locating as components of ONE Actor where that applies — a landmark with two sockets is one Actor, unambiguous by construction. Reach for Cohort only when the members are genuinely separate placeables.

`/Core/Cohort`

| Field | Type | | |
|---|---|---|---|
| `members` | `Array<Kind<Actor>>` | mutable · exposed | The grouped classes. |
| `scope` | `InstanceScope` | mutable · exposed | How tightly grouped. |
| `all_or_nothing` | `bool` | mutable · exposed | Whether a partial placement is acceptable. *Default: true.* |
| `ordered` | `bool` | mutable · exposed | Whether the order must be learnable. *Default: false.* |

### `Preference` — extends `Object`

A soft placement bias. Relaxable, and REPORTED when relaxed — nothing is loose by accident.

`/Core/Preference`

| Field | Type | | |
|---|---|---|---|
| `strictness` | `Strictness` | mutable · exposed | How hard the solver tries. *Default: PREFERRED.* |
| `weight` | `float` | mutable · exposed | Relative pull. *Default: 1.0.* |

### `Exclusion` — extends `Object`

What forbids() returns: a volume nothing may occupy, with declared escapes.

`/Core/Exclusion`

| Field | Type | | |
|---|---|---|---|
| `volume` | `Ref<CollisionBody>` | mutable · exposed | The excluded volume. |
| `unless` | `Array<Kind<Object>>` | mutable · exposed | Content permitted anyway. |
| `reason` | `String` | mutable · exposed | Prose for the trace. |

### `ScheduleRule` — extends `Object`

Ordering relative to other content.

`/Core/ScheduleRule`

| Field | Type | | |
|---|---|---|---|
| `relaxable` | `bool` | mutable · exposed | Whether the solver may break this when infeasible — and report it. *Default: true.* |

### `PlacedAfter` — extends `ScheduleRule`

Place me after this target, with a gap.

`/Core/PlacedAfter`

| Field | Type | | |
|---|---|---|---|
| `target` | `Kind<Object>` | mutable · exposed | What must come first. |
| `gap` | `Span` | mutable · exposed | How far after, in spheres. |

### `ExclusiveWith` — extends `ScheduleRule`

Prefer not to appear alongside this.

`/Core/ExclusiveWith`

| Field | Type | | |
|---|---|---|---|
| `other` | `Kind<Object>` | mutable · exposed | The other content. |
| `weight` | `float` | mutable · exposed | How strongly. |

### `Supersedes` — extends `ScheduleRule`

This replaces a base once available — the upgrade relationship.

`/Core/Supersedes`

| Field | Type | | |
|---|---|---|---|
| `base` | `Kind<Object>` | mutable · exposed | What this supersedes. |

### `SpherePin` — extends `ScheduleRule`

Pin to a sphere range. The first constraint about PACING rather than topology, and the one developers reach for soonest.

`/Core/SpherePin`

| Field | Type | | |
|---|---|---|---|
| `range` | `Span` | mutable · exposed | Permitted spheres. |

### `Rationale` — extends `Object`

Why the core did what it did. The trace is built from these.

`/Core/Rationale`

| Method | Returns | | |
|---|---|---|---|
| `subject()` | `Ref<Object>` | final | What this explains. |
| `layer()` | `int` | final | Which pipeline layer decided. |
| `inputs()` | `Array<Ref<Object>>` | final | What the decision read. |
| `explain()` | `String` |  | Prose. |
| `because()` | `Array<Ref<Rationale>>` | final | The upstream reasons, so a developer can walk back to the root cause. |

### `Context` — extends `Object`

**sealed** — content may not subclass this

The lens handed into every hook. Scope reads are FIELDS because they are free; queries are METHODS because they are not.

`/Core/Context`

| Field | Type | | |
|---|---|---|---|
| `world` | `ScopeHandle` | final | The World scope. |
| `reach` | `ScopeHandle` | final | The enclosing Reach. The ONLY legal use of the reach stem — everywhere else the noun is range and the verb is accessible. |
| `area` | `ScopeHandle` | final | The enclosing Area. |
| `space` | `ScopeHandle` | final | The enclosing Space, which is what bounds geometry queries. |
| `floor` | `ScopeHandle` | final | The enclosing Floor, which is what partitions accessibility. |
| `spatial` | `Ref<Actor>` | final | The content being considered at a position. |
| `slot` | `Ref<SpineSlot>` | final | The spine slot in play, or null outside a spine. Read-only. |
| `occupant` | `Occupant` | final | Who is being reasoned about. |
| `party` | `Array<Occupant>` | final | Everyone travelling together. |
| `held` | `Array<Unlock>` | final | Unlocks held. ROWS — one currency with grants() and HoldsRule. Already expanded through supersedes, so membership is a plain set test. |
| `sphere` | `int` | final | The current accessibility sphere. |
| `progression` | `float` | final | Normalised progress through the world. |
| `role` | `Role` | final | The role assigned to the content under consideration. |
| `layer` | `int` | final | Which pipeline layer is asking. |
| `fidelity` | `Fidelity` | final | How real the geometry currently is. |
| `tolerance` | `float` | final | The bounded error of this fidelity rung. The ladder is monotone, so this only ever shrinks. |
| `rng` | `Rng` | final | The ONLY randomness source. Anything else is unreplayable. |

| Method | Returns | | |
|---|---|---|---|
| `state_of(name: String)` | `String` | final | Read a declared world-state variable. |
| `pool(name: String)` | `Pool` | final | Read a declared resource pool. |
| `setting(name: String)` | `MetaValue` | final | Read a project setting. |
| `query()` | `Query` | final | The three-axis query builder: what to trace, what to consider, what to report. Declarative filters, never closures — a predicate callback survives neither the binding contract nor the palette. |
| `path_to(target: Ref<Object>, f: QueryFilter)` | `Ref<Path>` | final | Realise a path to a target. |
| `accessible(from: Vec3, to: Vec3, held: Array<Unlock>)` | `Trivalent` | final | Can an occupant holding these unlocks get from here to there? Trivalent, not bool, because the API must not be able to lie. |
| `within(measured: float, limit: float)` | `Trivalent` | final | Trivalent for METRIC questions. Dual bounds answer set membership; this answers 'is this ledge within 30 m', which every Span and Budget comparison actually asks. |
| `request(p: Ref<Preference>)` | `void` | final | Ask the solver for something, softly. |
| `note(r: Ref<Rationale>)` | `void` | final | Add a reason to the trace. |
| `send_message(text: String, channel: String, debug_only: bool)` | `void` | final | One-way notification to the host. Observational only, never affecting generation; debug_only messages are stripped entirely from a cooked build. |

### `ScopeHandle` — extends `Object`

**sealed** — content may not subclass this

A handle on one scope.

`/Core/ScopeHandle`

| Field | Type | | |
|---|---|---|---|
| `bounds` | `Aabb` |  | Extent. |
| `siblings` | `Array<ScopeHandle>` |  | Peers under the same parent. |
| `floors` | `Array<ScopeHandle>` |  | Floors inside this scope, if it is a Space. |
| `instances` | `Array<Ref<Actor>>` |  | Placed actors here. |
| `granted_here` | `Array<Unlock>` |  | Unlocks obtainable in this scope. |

| Method | Returns | | |
|---|---|---|---|
| `contains(a: Ref<Actor>)` | `bool` | final | Is this actor inside? |
| `accessible_from(other: ScopeHandle, held: Array<Unlock>)` | `Trivalent` | final | Can an occupant holding these get here from there? |
| `instances_of(kind: Kind<Object>, scope: InstanceScope)` | `Array<Ref<Object>>` | final | Placed content of a kind. Space and up — there is deliberately no floor-scoped instance query, because it would stop at a boundary the geometry does not stop at. |
| `dial(id: String)` | `float` | final | Read a numeric dial by its qualified <ClassName>.<DialName> id -- a scope handle may be any scope, so the owner is never implied. This is the DYNAMIC read; the typed one is the per-dial get node, which is picked and carries the dial's real type. Inherits OUTWARD-IN and an inner scope wins, so 'set saturation once at World scope' works. The trace records which scope supplied the value. |

---

## Structs

### `Vec2`

2D vector. components, length, normalized, dot, distance_to, arithmetic, and the ZERO/ONE constants.

`/Core/Vec2`

### `Vec3`

3D vector. components, length, normalized, dot, cross, distance_to, arithmetic, and the ZERO/ONE/UP/DOWN/FORWARD/RIGHT constants.

`/Core/Vec3`

### `Quaternion`

Rotation. from_euler, to_euler, slerp, mul.

`/Core/Quaternion`

### `Mat4`

4x4 matrix. origin, basis, apply, inverse, mul.

`/Core/Mat4`

### `Transform`

Position, rotation and scale. origin, basis, apply, inverse, mul.

`/Core/Transform`

### `Aabb`

Axis-aligned box. min, max, center, size, contains, intersects, expand, merge.

`/Core/Aabb`

### `Ray`

Origin, direction, at(t).

`/Core/Ray`

### `Plane`

Normal, d, distance_to(p).

`/Core/Plane`

### `Span`

An inclusive range. min, max, contains, clamp, length, overlaps, lerp, is_bounded, and UNBOUNDED. A Span DECLARES a range; a Route OBLIGES one.

`/Core/Span`

### `Unlock`

ONE ROW of an UnlockTableResource -- one atom of the progression lattice, something an occupant holds or knows. id, name, doc, supersedes. NOT A CLASS AND NOT A FILE: it carries no behaviour whatever, because every mechanical consequence belongs to a Component where affords/supports/judge can act on it. An unlock is an identity, and identity is all it is.

`/Core/Unlock`

### `Curve`

2D point data and NOT a resource — one row of a CurveTableResource. points, interpolation, sample, constant, ramp, from_points.

`/Core/Curve`

### `Dial`

A DEVELOPER-AUTHORED, named, tunable value owned by a Schematic or a Spine slot — how a host keeps fine-grained control over authored content at runtime. Identity is <ClassName>.<DialName>. Always exposed; always optional; the core ships none, and there is no such thing as a core dial. Holds a number, a hard range, a soft AdaptiveRange, an enum value, one curve, or a whole curve table whose named eval input it drives for every row. Resolves ONCE per generation pass, so it never changes underneath a decision mid-pass — which is why changing one is a different recipe and regenerates the world.

`/Core/Dial`

### `AdaptiveRange`

soft_min, hard_max, and target(available) computed from what is genuinely placeable. Falls below soft_min HONESTLY under content scarcity rather than padding with repeated filler or breaking outright.

`/Core/AdaptiveRange`

### `Quantity`

initial, max, consumable.

`/Core/Quantity`

### `Pool`

A resource an occupant draws on. capacity, reserve, and available(ctx) — the last is what every soft gate compares against, because capacity alone cannot answer how much is held at a given sphere.

`/Core/Pool`

### `Harm`

radius, severity 0..1, avoidable, continuous, mitigated_by, and NONE.

`/Core/Harm`

### `Support`

permitted (a Rule), max_slope, endurance (a Budget). Returned per occupant by Surface.supports.

`/Core/Support`

### `Approach`

distance (a Span), max_slope, surface. What an occupant needs at the near end of a traversal.

`/Core/Approach`

### `Occupant`

Who is standing, as a parameter. actor (null for the player), is_player, held, holds(kind), footprint.

`/Core/Occupant`

### `Hit`

A query result. valid, distance, point, normal, actor, component, island, polygon, triangle, surface, face, fidelity, and certainty. Fields below the achieved detail are ABSENT, which is checkable rather than sentinel-valued.

`/Core/Hit`

### `DirectionCone`

axis and angle. Replaces a three-value enum so that 'up, level or diagonally up, never down' is expressible.

`/Core/DirectionCone`

### `Kind`

A picked CLASS path. path, is_a, and defaults() — the core-owned class default, one per class, READ and never built by content. This is how a token's authored values are compared without instantiating anything.

`/Core/Kind`

### `Ref`

A live INSTANCE reference. Never interchangeable with Kind: a class never appears in a value position and a path never in a type position.

`/Core/Ref`

### `Tag`

A dotted hierarchical label, picked rather than typed.

`/Core/Tag`

### `TagQuery`

A tag match with an exact/inherited toggle. Why an eligible-surface list survives every future surface being added.

`/Core/TagQuery`

### `Quota`

How many of something may exist, per scope.

`/Core/Quota`

### `Diagnostic`

An editor-time finding. Never blocks generation.

`/Core/Diagnostic`

### `Rng`

The owned, forkable PRNG. Reached only through ctx.rng.

`/Core/Rng`

---

## Variants

### `Shape`

A parametric primitive. Collision is computed from parameters, never from tessellation — otherwise a visual LOD change silently alters generation.

`/Core/Shape`

| Method | Returns | | |
|---|---|---|---|
| `bounds()` | `Aabb` | final | Axis-aligned extent. |
| `volume()` | `float` | final | Enclosed volume. |
| `is_convex()` | `bool` | final | Convexity, which decides whether the collision cache survives scaling. |
| `contains(p: Vec3)` | `bool` | final | Point containment. |
| `closest_point(p: Vec3)` | `Vec3` | final | Nearest surface point. |
| `decompose()` | `Array<Shape>` | final | Convex decomposition. |
| `tessellate(lod: int)` | `Ref<MeshResource>` | final | Render geometry. Deliberately NOT the source of collision. |

### `SolidShape` — extends `Shape`

A shape with an inside. Supports booleans and a signed distance field.

`/Core/SolidShape`

| Method | Returns | | |
|---|---|---|---|
| `sdf(p: Vec3)` | `float` | final | Signed distance to the surface. |

### `SurfaceShape` — extends `Shape`

Zero thickness — no inside, so no sdf and no booleans.

`/Core/SurfaceShape`

### `CubeShape` — extends `SolidShape`

A box.

`/Core/CubeShape`

| Field | Type | | |
|---|---|---|---|
| `extents` | `Vec3` | mutable · exposed | Half-extents. |
| `bevel` | `float` | mutable · exposed | Edge bevel. *Default: 0.0.* |
| `segments` | `Vec3` | mutable · exposed | Tessellation density per axis. Render only. |

### `SphereShape` — extends `SolidShape`

A sphere.

`/Core/SphereShape`

| Field | Type | | |
|---|---|---|---|
| `radius` | `float` | mutable · exposed | Radius. |
| `segments` | `Vec2` | mutable · exposed | Tessellation density. |

### `HemisphereShape` — extends `SolidShape`

Half a sphere.

`/Core/HemisphereShape`

| Field | Type | | |
|---|---|---|---|
| `radius` | `float` | mutable · exposed | Radius. |
| `segments` | `Vec2` | mutable · exposed | Tessellation density. |
| `capped` | `bool` | mutable · exposed | Whether the flat face is closed. *Default: true.* |

### `ConeShape` — extends `SolidShape`

A cone.

`/Core/ConeShape`

| Field | Type | | |
|---|---|---|---|
| `radius` | `float` | mutable · exposed | Base radius. |
| `height` | `float` | mutable · exposed | Height. |
| `segments` | `int` | mutable · exposed | Radial tessellation. |
| `capped` | `bool` | mutable · exposed | Whether the base is closed. *Default: true.* |

### `CapsuleShape` — extends `SolidShape`

A capsule — the usual occupant proxy.

`/Core/CapsuleShape`

| Field | Type | | |
|---|---|---|---|
| `radius` | `float` | mutable · exposed | Radius. |
| `height` | `float` | mutable · exposed | Height of the cylindrical section. |
| `segments` | `Vec2` | mutable · exposed | Tessellation density. |

### `CylinderShape` — extends `SolidShape`

A cylinder, or a truncated cone when the radii differ.

`/Core/CylinderShape`

| Field | Type | | |
|---|---|---|---|
| `radius_top` | `float` | mutable · exposed | Top radius. |
| `radius_bottom` | `float` | mutable · exposed | Bottom radius. |
| `height` | `float` | mutable · exposed | Height. |
| `segments` | `int` | mutable · exposed | Radial tessellation. |
| `capped` | `bool` | mutable · exposed | Whether the ends are closed. *Default: true.* |

### `PrismShape` — extends `SolidShape`

An n-sided prism.

`/Core/PrismShape`

| Field | Type | | |
|---|---|---|---|
| `sides` | `int` | mutable · exposed | Number of sides. |
| `radius` | `float` | mutable · exposed | Circumradius. |
| `height` | `float` | mutable · exposed | Height. |
| `twist` | `float` | mutable · exposed | Rotation applied across the height. *Default: 0.0.* |

### `TorusShape` — extends `SolidShape`

A torus, optionally a partial arc.

`/Core/TorusShape`

| Field | Type | | |
|---|---|---|---|
| `major_radius` | `float` | mutable · exposed | Ring radius. |
| `minor_radius` | `float` | mutable · exposed | Tube radius. |
| `segments` | `Vec2` | mutable · exposed | Tessellation density. |
| `arc_sweep` | `float` | mutable · exposed | Swept angle. A full torus sweeps the whole circle. |

### `PipeShape` — extends `SolidShape`

A hollow cylinder.

`/Core/PipeShape`

| Field | Type | | |
|---|---|---|---|
| `inner_radius` | `float` | mutable · exposed | Bore radius. |
| `outer_radius` | `float` | mutable · exposed | Outer radius. |
| `height` | `float` | mutable · exposed | Height. |
| `segments` | `int` | mutable · exposed | Radial tessellation. |

### `ArchShape` — extends `SolidShape`

An arched opening — a doorway primitive.

`/Core/ArchShape`

| Field | Type | | |
|---|---|---|---|
| `width` | `float` | mutable · exposed | Opening width. |
| `height` | `float` | mutable · exposed | Total height. |
| `depth` | `float` | mutable · exposed | Wall thickness. |
| `arch_radius` | `float` | mutable · exposed | Radius of the arch crown. |
| `segments` | `int` | mutable · exposed | Arc tessellation. |

### `RampShape` — extends `SolidShape`

An inclined plane with thickness. A traversal primitive as much as a visual one.

`/Core/RampShape`

| Field | Type | | |
|---|---|---|---|
| `width` | `float` | mutable · exposed | Width. |
| `run` | `float` | mutable · exposed | Horizontal distance covered. |
| `rise` | `float` | mutable · exposed | Vertical distance covered. With run, this is what max_floor_slope is tested against. |
| `thickness` | `float` | mutable · exposed | Slab thickness. |
| `side_walls` | `bool` | mutable · exposed | Whether the sides are enclosed. *Default: false.* |

### `StairsShape` — extends `SolidShape`

A straight flight. The canonical intra-Space traversal edge.

`/Core/StairsShape`

| Field | Type | | |
|---|---|---|---|
| `width` | `float` | mutable · exposed | Width. |
| `steps` | `int` | mutable · exposed | Step count. |
| `step_rise` | `float` | mutable · exposed | Height of one step. |
| `step_run` | `float` | mutable · exposed | Depth of one step. |
| `risers` | `bool` | mutable · exposed | Whether the vertical faces are closed. *Default: true.* |
| `landing_at` | `int` | mutable · exposed | Step index a landing interrupts at. 0 = none. *Default: 0.* |
| `landing_run` | `float` | mutable · exposed | Depth of that landing. *Default: 0.0.* |

### `SpiralStairsShape` — extends `SolidShape`

A helical flight. The primitive that proves the library: it is the canonical multi-floor traversal AND the canonical is-this-a-ramp-or-steps collision question.

`/Core/SpiralStairsShape`

| Field | Type | | |
|---|---|---|---|
| `inner_radius` | `float` | mutable · exposed | Inner radius. |
| `outer_radius` | `float` | mutable · exposed | Outer radius. |
| `total_rise` | `float` | mutable · exposed | Total height climbed. |
| `steps` | `int` | mutable · exposed | Step count. |
| `sweep` | `float` | mutable · exposed | Total swept angle. |
| `clockwise` | `bool` | mutable · exposed | Handedness. *Default: true.* |
| `center_post` | `bool` | mutable · exposed | Whether a central column is present. *Default: true.* |

### `CompositeShape` — extends `SolidShape`

A boolean combination of solid shapes.

`/Core/CompositeShape`

| Field | Type | | |
|---|---|---|---|
| `operations` | `Array<BooleanOp>` | mutable · exposed | The ordered operation list. |

### `QuadShape` — extends `SurfaceShape`

A flat rectangle.

`/Core/QuadShape`

| Field | Type | | |
|---|---|---|---|
| `extents` | `Vec2` | mutable · exposed | Half-extents. |
| `segments` | `Vec2` | mutable · exposed | Tessellation density. |

### `TriangleShape` — extends `SurfaceShape`

A single triangle.

`/Core/TriangleShape`

| Field | Type | | |
|---|---|---|---|
| `a` | `Vec3` | mutable · exposed | First vertex. |
| `b` | `Vec3` | mutable · exposed | Second vertex. |
| `c` | `Vec3` | mutable · exposed | Third vertex. |

### `DiscShape` — extends `SurfaceShape`

A flat disc, optionally a sector.

`/Core/DiscShape`

| Field | Type | | |
|---|---|---|---|
| `radius` | `float` | mutable · exposed | Radius. |
| `segments` | `int` | mutable · exposed | Radial tessellation. |
| `arc_start` | `float` | mutable · exposed | Start angle. *Default: 0.0.* |
| `arc_sweep` | `float` | mutable · exposed | Swept angle. |

### `EllipseShape` — extends `SurfaceShape`

A flat ellipse.

`/Core/EllipseShape`

| Field | Type | | |
|---|---|---|---|
| `radius` | `Vec2` | mutable · exposed | Semi-axes. |
| `segments` | `int` | mutable · exposed | Tessellation density. |

### `DistanceCost` — extends `Cost`

Metres.

`/Core/DistanceCost`

| Field | Type | | |
|---|---|---|---|
| `limit` | `float` | mutable · exposed | World units. |

### `TimeCost` — extends `Cost`

Seconds. Every TimeCost is a distance divided by player_profile.speed, which is why that setting is not optional.

`/Core/TimeCost`

| Field | Type | | |
|---|---|---|---|
| `limit` | `float` | mutable · exposed | Seconds. |
| `speed` | `float` | mutable · exposed | World units per second. Without it there is no way to turn seconds into a reachable distance. |

### `PoolCost` — extends `Cost`

Draw against a named resource pool at a rate. How a soft gate is a magnitude rather than a rule — the solver can trade it off instead of treating it as impassable.

`/Core/PoolCost`

| Field | Type | | |
|---|---|---|---|
| `pool` | `String` | mutable · exposed | Which declared resource. |
| `limit` | `float` | mutable · exposed | How much of the pool may be drawn. |
| `rate` | `float` | mutable · exposed | Draw per world unit travelled. |

### `NamedBudget` — extends `BudgetRef`

A row of the project's BudgetBook — retune it in one place and every site naming it moves.

`/Core/NamedBudget`

| Field | Type | | |
|---|---|---|---|
| `budget` | `Ref<Budget>` | mutable · exposed | Which named budget. |

### `InlineBudget` — extends `BudgetRef`

A cost authored at this site. Right for a one-off; a magic number if it repeats — and because inline and named are told apart, a tool can notice when it has.

`/Core/InlineBudget`

| Field | Type | | |
|---|---|---|---|
| `cost` | `Cost` | mutable · exposed | What it costs, here. |

### `Cost`

**sealed** — content may not subclass this

A kind and a limit, with NO accounting — Distance(m), Time(s) or Pool(pool, rate). What a Budget is a named instance of, and what an interaction spends.

`/Core/Cost`

### `MetaValue`

**sealed** — content may not subclass this

What a metadata value may hold. A CLOSED set, deliberately: an open Any would let content stash a handle the fingerprint cannot see, and the first symptom would be a world that fails to reproduce with no visible cause.

`/Core/MetaValue`

### `BoolMeta` — extends `MetaValue`

A yes or no.

`/Core/BoolMeta`

### `IntMeta` — extends `MetaValue`

A whole number. i32, not i64: a JavaScript number is exact only below 2^53, and metadata crosses the binding seam constantly.

`/Core/IntMeta`

### `FloatMeta` — extends `MetaValue`

A real number.

`/Core/FloatMeta`

### `StringMeta` — extends `MetaValue`

Text.

`/Core/StringMeta`

### `Vec3Meta` — extends `MetaValue`

A position or direction.

`/Core/Vec3Meta`

### `TransformMeta` — extends `MetaValue`

A placement.

`/Core/TransformMeta`

### `ArrayMeta` — extends `MetaValue`

An ordered list of metadata values.

`/Core/ArrayMeta`

### `MapMeta` — extends `MetaValue`

A named map of metadata values.

`/Core/MapMeta`

### `RefMeta` — extends `MetaValue`

A reference to something with identity.

`/Core/RefMeta`

### `BudgetRef`

**sealed** — content may not subclass this

'This budget' — Named(Ref<Budget>) into the project's book, or Inline(Cost) authored at the site. BOTH forms stay and stay DISTINGUISHABLE: forcing a one-off through the book is ceremony for a number used once, and because the two are told apart a tool can notice the same inline number in twelve places and offer to extract it.

`/Core/BudgetRef`

---

## Enums

### `Face`

Which face of a bounding box something presents or mounts against.

`/Core/Face`

| Value | |
|---|---|
| `POS_X` |  |
| `NEG_X` |  |
| `POS_Y` | Up. The default mount face — a floor-standing thing. |
| `NEG_Y` | Down. A ceiling mount. |
| `POS_Z` |  |
| `NEG_Z` |  |

### `Role`

What a placement turned out to be. An OUTPUT, assigned after the search from what it ran into — never declared.

`/Core/Role`

| Value | |
|---|---|
| `DECORATION` | No mechanical consequence. |
| `OBSTACLE` |  |
| `TRAVERSAL` |  |
| `GATE` |  |
| `LANDMARK` |  |

### `ItemClass`

What kind of reward something is. An INPUT the developer declares.

`/Core/ItemClass`

| Value | |
|---|---|
| `PROGRESSION` | May appear in logic. The conservative default. |
| `USEFUL` | Tunes route difficulty, spends slack, never gates. |
| `BONUS` | Rewards optional exploration. Auto-assigned to anything accessible solely through a relaxation. |
| `FILLER` | Satisfies density. Currency, ammo, consumables. |

### `CollisionLayer`

Which collision layer a body belongs to.

`/Core/CollisionLayer`

| Value | |
|---|---|
| `HULL` |  |
| `STATIC` |  |
| `DYNAMIC` |  |

### `QueryFilter`

Coarse query scoping.

`/Core/QueryFilter`

| Value | |
|---|---|
| `ALL` |  |
| `NONE` |  |
| `WORLD` |  |
| `PLACED` |  |

### `CollisionMode`

▶ **PROPOSED** — may change or be removed

PROPOSED. How collision is derived from geometry. The design names the enum but not its values; these are the minimum set the pipeline needs and must be confirmed when mesh import lands.

`/Core/CollisionMode`

| Value | |
|---|---|
| `NONE` | Visible, never collidable. |
| `CONVEX_HULL` |  |
| `DECOMPOSED` |  |
| `TRIANGLE_MESH` |  |
| `AUTHORED` | Use a separately authored collision body rather than deriving one. |

### `Replenish`

Whether and how a supply comes back.

`/Core/Replenish`

| Value | |
|---|---|
| `NEVER` |  |
| `ON_REENTER` |  |
| `FROM_SOURCE` |  |
| `ON_TIMER` |  |

### `InstanceScope`

How wide an instance query or effect reaches. There is deliberately NO FLOOR member: a floor-scoped instance query would stop at a boundary the geometry does not stop at.

`/Core/InstanceScope`

| Value | |
|---|---|
| `SPATIAL` |  |
| `SPACE` |  |
| `AREA` |  |
| `REACH` |  |
| `WORLD` |  |

### `BooleanOp`

Constructive solid geometry operations.

`/Core/BooleanOp`

| Value | |
|---|---|
| `UNION` |  |
| `SUBTRACT` |  |
| `INTERSECT` |  |

### `ResolveState`

How committed a node is. A subtree may lag but never lead.

`/Core/ResolveState`

| Value | |
|---|---|
| `PROJECTED` | A revisable forecast. The only state from which a node may be removed. |
| `RESERVED` | Committed to exist, with an envelope claimed. Removable only by recorded backtracking. |
| `REALIZED` | Built and frozen. |

### `Detail`

How much a query wants back. Detail is what you ASK for; fidelity is what EXISTS.

`/Core/Detail`

| Value | |
|---|---|
| `SCOPE` | Which room. Available from L1. |
| `COLLIDER` | Which box, which face. From L2. |
| `INSTANCE` | Which placed actor, with its content path and metadata. From L2. |
| `ISLAND` | Which mesh island. From L3. |
| `POLYGON` | Which hull polygon or occupancy cell. From L3. |
| `TRIANGLE` | Which triangle, with an interpolated normal. From L4. |

### `Fidelity`

How real the geometry is. The ladder is monotone — each rung only tightens, so tolerance only shrinks.

`/Core/Fidelity`

| Value | |
|---|---|
| `ENVELOPE` | Tolerance is the envelope's own slack. |
| `HULL` | Tolerance is the contouring tolerance. |
| `GEOMETRY` | Tolerance is zero. |

### `Strictness`

How hard a spine slot or preference must hold. Every relaxation is declared; nothing is loose by accident.

`/Core/Strictness`

| Value | |
|---|---|
| `REQUIRED` | Must hold; if it cannot, generation fails with a diagnostic. |
| `PREFERRED` | Strongly biased; may be relaxed when infeasible, and is REPORTED when it is. |
| `OPTIONAL` | The generator decides freely; absence is expected. |

### `SkipPolicy`

Per-lock sequence-break policy. A designer marks two or three gates, not two hundred.

`/Core/SkipPolicy`

| Value | |
|---|---|
| `TOLERATED` | A path exists; other emergent paths are fine. The default, because real games ship tolerated skips deliberately. |
| `EXACT` | Report every alternative route found. |
| `GUARDED` | Actively verify no alternative exists at that sphere, and fail loudly if one does. Also what decides whether a discovered shortcut may be adopted. |

### `Interpolation`

How a curve row interpolates between keys. Declared PER ROW — UE fixes it per table only because a CSV has one header row, which JSON does not.

`/Core/Interpolation`

| Value | |
|---|---|
| `CONSTANT` |  |
| `LINEAR` |  |
| `CUBIC` |  |

### `Trivalent`

Three-valued truth. A confident wrong answer is worse than an admitted unknown, so the API returns this rather than a bool wherever geometry is still approximate.

`/Core/Trivalent`

| Value | |
|---|---|
| `YES` |  |
| `NO` |  |
| `AMBIGUOUS` | Inside the ambiguous band. Resolve, never guess — and a decision that returns this re-asks at the next fidelity rung by construction. |

