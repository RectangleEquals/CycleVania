# 02 · Determinism

> One `(WorldSeed, ReachRequestLog)` produces byte-identical worlds forever, on every platform.
> This is what makes co-op possible (server generates, clients reconstruct), replays possible,
> and every bug reproducible. It is the least negotiable property in the whole design.

## The determinism law

1. **No `Math.random`, anywhere in generation.** All randomness flows from one deterministic `Rng`
   owned by `WorldComposer` and threaded downward via forks.
2. **No host trigonometry for generation state.** `Math.sin/cos/tan/atan2` differ at the bit level
   across JS engines. Use the deterministic minimax-polynomial implementations (`dsin`, `dcos`,
   `datan`, `datan2`) for anything that touches generated state. `Math.floor`, `Math.sqrt`,
   `Math.hypot`, `Math.abs`, `Math.min/max`, `Math.imul`, `Math.exp` and plain arithmetic are
   IEEE-754-exact and allowed.
3. **Fork, never share, RNG streams — keyed by stable identity.** `rng.fork(label)` derives an
   independent child stream *without advancing the parent*, so adding a draw in one subsystem never
   perturbs another. Labels must be **stable identities** (a role, an id, a coordinate, a request
   fingerprint) — never an array index — so an unrelated content edit doesn't reshuffle every
   downstream seed.
4. **Fork by everything that varies the outcome.** A Reach's fork label includes every field of its
   `ReachRequest` that can change the result: `reach${i}:mods[${sortedModifierIds}]:econ[${overrideHash}]:tpl[${templateHash}]`.
   The player's and host's choices are part of the identity being forked on.
5. **Previews and plans are read-only.** `previewReachEnvelope`, `computeVirtualSchedule`, and any
   other lookahead query may evaluate the same curves, but must never draw from a stream that real
   generation later consumes — merely *asking* what's ahead must never change what's ahead.
6. **Host callbacks must be pure.** Facet evaluators (`MagnitudeFacet.evaluate`,
   `TagFacet.evaluate`, `ResourceFacet.capacity`) and weighting functions
   (`AreaCountConfig.weights`, `powerWeight`, …) are host code CycleVania calls *during*
   generation. Given the same inputs they must return the same value, forever: no wall-clock reads,
   no `Math.random`, no mutable captured state. This is a distinct determinism surface (executable
   host code, not just data) and is validated in tests by double-running generation.
7. **Golden-vector parity for hand-ported primitives.** The RNG core, trig, and noise pin their
   exact outputs in `math/golden/*.json`; CI re-asserts byte equality. A one-bit drift here desyncs
   every downstream layer invisibly.
8. **CI grep guard.** The build fails on `Math\.random` or `Math\.(sin|cos|tan|atan)` anywhere in
   `core/src` (test files excepted where they compare against host math deliberately).

## The deterministic primitives

These are specifications, not suggestions — each has existing proven code to port
([01](./01-architecture.md) module table) and golden vectors to hold it to.

### `Rng`

- **Core**: sfc32 (four 32-bit lanes), seeded via FNV-1a hashing of the seed string, expanded with
  splitmix32. All integer math via `Math.imul`/`>>> 0` (exact in JS).
- **API**: `next(): number` (float in [0,1)), `int(maxExclusive)`, `range(min, max)`,
  `chance(p): boolean`, `pick<T>(arr)`, `weighted<T>(entries: {item,T; weight: number}[])`,
  `shuffle<T>(arr)` (Fisher–Yates, returns a copy), `triangular(min, max)` (sum of two uniforms —
  used by the complexity jitter), `fork(label: string): Rng`.
- **`fork` semantics**: child seed = FNV-1a over (parent's seed identity + label). Forking never
  advances the parent's state; forking the same label twice yields identical children.

### Deterministic trig

Minimax polynomial approximations over a range-reduced argument: `dsin`, `dcos` (argument reduced
to [−π, π] by exact multiples of 2π), `datan`, `datan2` (quadrant-correct). Accuracy target ≤ 1e-7
absolute error — far below any geometric tolerance in the pipeline — with the *same* bits on every
platform, which host `Math.sin` cannot promise. Helpers: `yawFromDirection(v)`, `yawBasis(yaw)`.

### Deterministic 3D noise

Seeded gradient noise: a permutation table derived from an `Rng` fork, gradients from a fixed
12-direction set (no trig needed), smootherstep interpolation. `noise3(p): number` in [−1, 1];
`fbm3(p, octaves, lacunarity, gain)` for fractal detail. Used by hull displacement
([08](./08-volumetric-composition.md)) and dressing ([09](./09-naturalization-and-kit.md)).

### Deterministic QEF solver

The dual-contouring vertex placement ([09](./09-naturalization-and-kit.md)) solves a 3×3
least-squares system (normal equations, symmetric accumulation, fixed-order arithmetic, singular
fallback to the mass point, result clamped to the cell). Pure arithmetic — deterministic by
construction — but pinned by geometry golden tests anyway because its output feeds content hashes.

## The reproducibility unit: `(WorldSeed, ReachRequestLog)`

`WorldSeed` alone is *not* the unit, because Reaches are generated on demand with player/host
choices folded in ([04](./04-worlds-reaches-and-pacing.md)). The unit is:

- **`WorldSeed`** — the opaque seed string, plus
- **`ReachRequestLog`** — the ordered list of every full `ReachRequest` issued so far
  (`reachIndex`, `chosenModifiers`, any overrides).

Two runs sharing both get a byte-identical World — **without either ever materializing the full,
uncapped World anywhere**. This is exactly the shape of a seeded infinite-terrain generator, and
the resemblance is structural, not cosmetic:

| Infinite terrain (chunked noise) | CycleVania (on-demand Reaches) |
|---|---|
| `heightAt(x,y) = noise(seed, x, y)` — pure function of coordinates | `previewReachEnvelope(i)` — pure function of `(seed, i, realized history)` |
| A chunk never depends on generating neighbors first | A preview never depends on generating later Reaches first |
| Query terrain 500 chunks away for free | Preview Reach 40's envelope without Reaches 5–39 existing |
| Only visited chunks persist | Only realized Reaches persist |
| Seed + params reproduce any chunk forever | Seed + request log reproduce any realized Reach forever |

The one real difference: the complexity formula's lookbehind term makes Reach *i* causally depend
on Reach *i−1* being realized. That's safe in practice — whatever host trigger issues the request
for Reach *i* necessarily lives inside an already-realized Reach — and previews never take that
dependency (they read only already-fixed facts), which keeps them O(1) at any distance.

## Registry fingerprint & generation versioning

Determinism is conditional on *inputs* not silently changing. The output records everything the
guarantee is conditional on ([10](./10-output-contract.md) meta):

- **`registryFingerprint`** — a stable content hash (FNV-1a over a canonical serialization) of the
  entire validated registry: catalogs, configs, templates, modifier defs, biome packs, hull
  archetypes, fidelity profile. Host callbacks can't be hashed, so each callback-bearing def
  carries a host-declared `revision: number` the fingerprint folds in — bumping it is the host's
  contract that behavior changed. (Non-bumped behavioral changes violate rule 6 and are caught by
  the double-run test.)
- **`generationVersion`** — CycleVania's own algorithm version. Any change to any generation
  algorithm that alters output for the same inputs bumps it. Same seed + same fingerprint +
  different `generationVersion` ⇒ legitimately different worlds; tooling can say *why* a world
  changed instead of leaving the host guessing.
- **`requestIdentity`** per Reach — the request's canonical hash, echoed into that Reach's meta.

## Geometry buffer determinism

Float geometry survives determinism because every producing algorithm is deterministic — but
*hashing* geometry (kit dedup, golden tests) additionally requires stable canonical form:

- Kit piece coordinates are **rounded to 1e-3** before hashing and before emission (cell-local
  coordinates; see [09](./09-naturalization-and-kit.md)). The rounding is part of the output
  contract, not a test convenience.
- Vertex and index order are fully determined by grid iteration order (x → y → z, fixed corner and
  edge tables). No hash maps with nondeterministic iteration anywhere in meshing — plain arrays and
  `Map` (insertion-ordered) only.
- All accumulations that feed output (QEF sums, normal averaging) use fixed iteration order;
  never reduce over an unordered set.

## Fork-label conventions (normative)

| Stream | Label pattern |
|---|---|
| World length draw | `world-length` |
| Virtual schedules | `virtual-schedule:gadgets` · `virtual-schedule:puzzles` |
| Reach entropy jitter | `reach-entropy:${i}` |
| A Reach's generation root | `reach${i}:mods[…]:econ[…]:tpl[…]` (full request identity) |
| Template interpretation | `${reachRoot}:template` |
| Gadget / puzzle schedulers | `${reachRoot}:gadget-schedule` · `${reachRoot}:puzzle-schedule` |
| Assumed fill | `${reachRoot}:fill` |
| Per Area | `${reachRoot}:area:${regionId}` |
| Per Space | `${areaRoot}:space:${spaceId}` |
| Anchor scatter | `${spaceRoot}:anchors:${kindId}` |
| Dressing | `${areaRoot}:dress` |

Every stream consumed anywhere in generation appears in this table (extended as modules are built);
an unlisted ad-hoc fork is a review error. Identities are ids/roles — never indices — so removing
one catalog entry perturbs only draws that genuinely depend on it.

## What can legitimately change a world

For host-facing docs and tooling, the complete list. Anything else changing the output is a bug:

1. A different `WorldSeed`.
2. A different `ReachRequestLog` (different player modifier choices, different host overrides,
   different request order for branching worlds).
3. A registry data change (⇒ new `registryFingerprint`).
4. A host callback behavior change (⇒ host bumps `revision` ⇒ new fingerprint).
5. A CycleVania algorithm change (⇒ new `generationVersion`).
