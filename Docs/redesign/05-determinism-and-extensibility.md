# 05 · Determinism & extensibility

> The rules that keep one `WorldSeed` producing byte-identical worlds forever — including, new in
> this revision, why generating Reaches **lazily/on-demand** (never the whole World up front)
> doesn't cost any of that determinism — and the full surface a host project touches to plug its
> own game into CycleVania.

## Determinism rules (apply to every layer, no exceptions)

1. **No host `Math.random`, anywhere in generation.** All randomness flows from one deterministic
   `Rng` owned by `WorldComposer` and threaded downward.
2. **No host trigonometry for generation state.** `Math.sin/cos/atan2` differ at the bit level
   across JS engines; use a deterministic minimax-polynomial trig implementation instead so a
   server and every client reconstruct byte-identical worlds (this is what makes co-op / replays /
   cross-platform parity possible at all).
3. **Fork, never share, RNG streams — and fork by every stable input that actually varies the
   outcome.** `rng.fork(label)` derives an independent child stream *without advancing the parent*.
   The label must be a **stable identity** (a role, an id, a coordinate) — never an array index —
   so an unrelated content edit doesn't reshuffle every downstream seed. Critically, once Reach
   modifiers exist ([02](./02-composers-and-complexity.md)), a Reach's fork label must include
   *every field of its `ReachRequest` that can vary the outcome* — not just `reachIndex`, but the
   chosen modifier set and any `gadgetEconomyOverride`/`template` override too
   (`reach${i}:mods[${sortedModifierIds}]:econ[${economyOverrideHash}]`) — the player's and host's
   choices are both part of what varies the outcome, so both belong in the identity being forked on.
4. **Golden-vector parity for anything hand-ported.** If any deterministic primitive (RNG core,
   trig, noise) is reimplemented from a reference, pin its exact outputs and re-assert them in CI —
   a silent one-bit drift here desyncs every downstream layer invisibly.
5. **Lookahead/preview queries must stay read-only.** `previewReachEnvelope` ([02](./02-composers-and-complexity.md))
   may *evaluate* the same curve/entropy/modifier math used elsewhere, but must never itself draw
   from a stream that real generation later consumes — otherwise merely *asking* what's ahead would
   change what's ahead.
6. **Host-supplied Facet evaluators ([03](./03-locks-keys-and-gadgets.md)) must be pure.** A
   `MagnitudeFacet.evaluate`/`TagFacet.evaluate`/`ResourceFacet.capacity` callback is host code
   CycleVania calls *during* generation — the same purity requirement as everything else in this
   list applies to it: given the same `FacetContext`, it must return the same value, forever, with
   no wall-clock reads, no host-side `Math.random`, and no mutable state carried between calls. This
   is a new *kind* of determinism surface (executable host code, not just host data), so it's worth
   calling out on its own rather than assuming rule 1 already covers it implicitly.

## Lazy generation & the infinite-terrain analogy

This is worth stating as its own principle because it's the piece that makes on-demand Reach
generation ([02](./02-composers-and-complexity.md)) safe rather than merely convenient.

**The reproducibility unit is `(WorldSeed, ReachRequestLog)`, not `WorldSeed` alone.**
`ReachRequestLog` is the ordered list of every full `ReachRequest` actually issued so far via
`requestReach` — `reachIndex`, `chosenModifiers`, and any `gadgetEconomyOverride`/`template`
override, per Reach (see [02](./02-composers-and-complexity.md)). Two runs that share a
`WorldSeed` and replay the same requests get a byte-identical World — **without either of them ever
needing the full, uncapped
World to exist anywhere.** A host-declared `MaxReachCount` (if any) only bounds how large
`reachIndex` may legally grow; it is never a generation-*eagerness* knob, and nothing about
determinism requires it to be small.

This is exactly the shape of a well-built seeded infinite-terrain generator, and the resemblance is
not a coincidence — both problems reduce to the same requirement:

| Infinite terrain (chunked noise) | CycleVania (on-demand Reaches) |
|---|---|
| `heightAt(x, y) = noise(worldSeed, x, y)` — a pure function of coordinates | `previewReachEnvelope(i)` — a pure function of `(worldSeed, i, realized history)` |
| A chunk's height never depends on *generating* neighboring chunks first | A Reach's preview never depends on *generating* later Reaches first |
| Player can query "what does the terrain look like 500 chunks away" for free | Host can preview Reach 40's envelope without Reaches 5–39 having been played |
| Only *already-visited* chunks need to be persisted/streamed | Only *already-realized* Reaches need to be persisted |
| A world seed + noise params fully reproduce any chunk, forever | A `WorldSeed` + `ReachRequestLog` fully reproduce any realized Reach, forever |

The one place the analogy has a real (not superficial) difference: terrain noise is typically
*commutative* — chunk (5,5) doesn't causally depend on chunk (4,5) having been sampled. CycleVania's
`RealizedCeiling(i-1)` lookbehind term ([02](./02-composers-and-complexity.md)) makes Reach `i`
*causally* depend on Reach `i-1` already being concretely realized. That's fine in practice: whatever
gates a host's own request-trigger for Reach `i` will itself already require Reach `i-1` to be
realized (the player has to have gotten there first), so the ordering is guaranteed by the host's
fiction, not by CycleVania. `previewReachEnvelope`, by contrast, never takes that dependency — it
only reads facts that already exist by construction, which is what keeps it O(1) and
terrain-analogy-safe even for far-future indices.

## The registries — the entire host-integration surface

This is deliberately small. If a host needs to reach past these to make CycleVania do something
game-specific, that's a sign a new named contract belongs here, not a one-off hook.

| Registry | Defined in | Purpose |
|---|---|---|
| `GadgetCatalog` | [03](./03-locks-keys-and-gadgets.md) | `CapabilityDef[]` + `GadgetDef[]` — Facets, `held`, `powerWeight`, optional `guarantee` |
| `GadgetEconomyConfig` | [03](./03-locks-keys-and-gadgets.md) | Baseline `{ min, max }` progression items per Reach |
| `PuzzleCatalog` | [07](./07-puzzles-and-challenges.md) | `PuzzleDef[]` — `scope`, `condition`, `outcome`, `class`, `powerWeight`, optional `guarantee` |
| `PuzzleEconomyConfig` | [07](./07-puzzles-and-challenges.md) | Baseline `{ min, max }` puzzles per Reach — independent of `GadgetEconomyConfig` |
| `LockVocabulary` | [07](./07-puzzles-and-challenges.md) | Named `PuzzleDef` recipes → `Rule` builders for common patterns |
| `BiomePack` set | [04](./04-spatial-composition-and-sockets.md) | Palette/materials/hazards/dressing/noise per biome, plus sub-biome blend weights |
| `ContentAnchorKind` set | [04](./04-spatial-composition-and-sockets.md) | Per-kind density/separation/clearance dials for content Socket placement |
| `GeometryKit` | [04](./04-spatial-composition-and-sockets.md) | The modular piece grammar L4 assigns per surface role |
| `ComplexityConfig` | [02](./02-composers-and-complexity.md) | `BaseCeiling`, `K_MUL`, `K_ADD`, `TIER_SIZE`, jitter/clamp bounds |
| `AreaCountConfig` | [02](./02-composers-and-complexity.md) | `{ min, max, weights? }` — how many Areas a Reach draws |
| `WorldLengthPolicy` | [02](./02-composers-and-complexity.md) | `{ min, max, weights? }` — how many Reaches a World draws, once, up front |
| `ReachTemplate` set | [01](./01-mission-graph.md) | The macro-shape(s) a World draws Reaches from |
| `ReachModifierCatalog` + `ReachModifierPolicy` | [02](./02-composers-and-complexity.md) | The full modifier pool, its depth-driven growth, and the optional→mandatory required-count ramp |

## The "never assumes gameplay" checklist

Before adding anything to CycleVania itself, check: does this require knowing what a Gadget *does*,
what a puzzle *is*, what a biome *looks like*, what a Reach modifier is called in-fiction, or what
ritual/UI triggers a `ReachRequest`? If yes, it belongs in a registry entry — or the host's own
trigger code around `requestReach` — not in CycleVania's own code, its docs, or its type names.
Every doc in this folder should remain fully readable, and every algorithm in them fully
exercisable in tests, without a single real Gadget name, puzzle mechanic, biome name, or Reach
modifier's in-fiction name ever appearing. (An earlier draft of this doc briefly violated this rule
by naming a specific host's in-fiction terms directly in the core text — worth remembering as a
concrete example of the failure mode this checklist exists to catch.)

## Validation strategy

- **Solvability soak** — generate N seeds (hundreds+) end-to-end and assert `isSolvable` on every
  one; this is the load-bearing regression test for the entire mission-graph layer.
- **Determinism golden runs** — same `(WorldSeed, ReachRequestLog)`, two independent generation
  passes, byte-identical descriptor output. Include at least one golden run with a non-empty
  modifier history, not just the zero-modifier case.
- **Budget-adherence invariants** — assert `FinalCeiling` never exceeds `ABSOLUTE_HARD_MAX`
  regardless of stacked modifiers, and assert on **rolling-average** monotonicity across
  `ReachLevel` tiers (never on adjacent-Reach pairs).
- **Preview purity** — assert `previewReachEnvelope(i)` called twice in a row (with nothing else
  happening in between) returns identical results, and assert calling it never advances any RNG
  stream that a subsequent real `requestReach(i)` then consumes.
- **Scheduler-distribution tests** — over many seeds, a high-`powerWeight` Gadget's mean placement
  `ReachLevel` should be measurably later than a low-`powerWeight` Gadget's, without ever being
  literally impossible early.
- **Pity/guarantee eventually-fires** — for any Capability with a `guarantee.withinReachLevels`, soak
  many seeds and assert it is placed by that bound in every one, and assert a forced placement never
  causes `validateGraph` to fail (a pity-forced item still has to land somewhere reachable).
- **Solvability locality** — assert `validateGraph`/`assumedFill` for Reach `i` never reads any state
  from Reach `i+1` or later; a passing test here is the concrete proof that "generated on demand"
  and "provably solvable" don't actually conflict (see [01](./01-mission-graph.md)).
- **Content-anchor overlap** — for a generated Space, assert no two accepted content anchors (of any
  kind) are closer than the stricter of their two `minSeparation`s, and none intrude into a
  structural Socket's `clearanceFromStructural` buffer or back through the Hull boundary
  ([04](./04-spatial-composition-and-sockets.md)).
- **Virtual-schedule purity and final-sweep completeness** — for a bounded `WorldLengthPolicy`,
  assert `computeVirtualSchedule` returns identical results across repeated calls with nothing else
  happening in between (same purity requirement as `previewReachEnvelope`), and soak many seeds to
  assert every `progression`-class Capability *and* every `required`-class Puzzle
  ([07](./07-puzzles-and-challenges.md)) is placed by Reach `L - 1` regardless of how much either
  plan drifted earlier — the concrete proof that the final-Reach sweep actually backstops both
  pools independently ([02](./02-composers-and-complexity.md)).
- **Pool independence** — assert that changing the Puzzle catalog, `PuzzleEconomyConfig`, or any
  Puzzle's `guarantee` never perturbs the Gadget pool's scheduling outcomes for an otherwise
  identical `(WorldSeed, ReachRequestLog)`, and vice versa — the concrete proof that
  `"gadget-schedule"`/`"puzzle-schedule"` and their respective virtual-schedule fork namespaces
  ([02](./02-composers-and-complexity.md)) are genuinely independent, not just labeled as such.

## Suggested build order for a new contributor

1. **Rule + graph core** ([01](./01-mission-graph.md)) — `Rule`, `reachableRegions`, `isSolvable`,
   with zero spatial concerns at all. Fully unit-testable with tiny hand-written graphs.
2. **`assumedFill`** ([01](./01-mission-graph.md)) — layer placement on top of the graph core; test
   with the solvability soak immediately.
3. **`ReachTemplate` interpretation** ([01](./01-mission-graph.md)) — turn data into a concrete
   graph; this is where `validateGraph`'s fail-loudly precondition earns its keep.
4. **Complexity budget + the `ReachIndex` formula** ([02](./02-composers-and-complexity.md)) — pure
   math, testable with the worked-example table before any Space or modifier exists.
5. **`ReachRequest` + `ReachModifierCatalog`/`ReachModifierPolicy`** ([02](./02-composers-and-complexity.md))
   — wire on-demand generation and the fourth budget term on top of step 4's now-proven math; this
   is also where `previewReachEnvelope` and its purity tests belong.
6. **`AreaCountConfig` + `WorldLengthPolicy` + the virtual schedule** ([02](./02-composers-and-complexity.md))
   — the ranged-count pattern applied at both the Reach and World level, and the cross-Reach pacing
   mechanism on top; test the final-sweep completeness soak from the validation strategy above
   before moving on.
7. **`GadgetCatalog` + scheduler** ([03](./03-locks-keys-and-gadgets.md)) — coupled to steps 2/3;
   test the eligibility-distribution property, not individual outcomes.
8. **`PuzzleCatalog` + its own scheduler** ([07](./07-puzzles-and-challenges.md)) — deliberately
   *after* step 7 so the pool-independence test above has something real to check against; reuses
   every piece of step 7's machinery, just against a second catalog and fork namespace.
9. **`SpaceComposer` + Sockets** ([04](./04-spatial-composition-and-sockets.md)) — the first layer
   that touches real coordinates; everything above it should already be fully solved and tested.
10. **L4 naturalization** ([04](./04-spatial-composition-and-sockets.md)) — purely a finishing pass;
    swappable independently of everything else once the Hull contract is stable.

Building in this order means every layer below the one you're working on is already proven —
exactly the point of separating the layers in [00](./00-overview.md) in the first place.
