# M03 · World layer — requests, modifiers, complexity, previews, schedules

**Goal**: the `world/` module — `WorldComposer` with on-demand `requestReach`, Reach modifiers +
`DialPatch`, the four-term complexity formula + hazard/reward baselines, pure previews, the
generic virtual schedule, `WorldLengthPolicy`, and `ReachPortal`s. Content *selection* is
injected as a strategy in this milestone (M04 supplies the real one), so the world layer is fully
testable now.

**Required reading**: [redesign 04 (all)](../redesign/04-worlds-reaches-and-pacing.md);
[redesign 02 §Fork-label conventions, §Reproducibility unit](../redesign/02-determinism.md).

**Prerequisites**: M02 green.

## Phase 3.1 — Pure math: `complexity.ts`, `length-policy.ts`

`complexity.ts` — `ComplexityConfig` (all nine constants from redesign 14's table) with shipped
defaults (`BaseCeiling 100`, `K_MUL 0.45`, `K_ADD 5`, `TIER_SIZE 3`, `JITTER_FRAC 0.08`,
`LOOKBEHIND_PULL 0.35`, `MIN_CEILING 60`, `HARD_MAX 400`, `ABSOLUTE_HARD_MAX 480` — these
reproduce the worked-example table in redesign 04 within rounding; if they don't, adjust the
table's jitter column comment, not the formula). Functions:

```ts
export function reachLevel(i: number, cfg: ComplexityConfig): number;
export function expectedCeiling(i: number, cfg: ComplexityConfig): number;
export function actualCeiling(i: number, realizedPrev: number | undefined, rng: Rng, cfg: ComplexityConfig): number;
export function finalCeiling(actual: number, mods: ReachModifierDef[], cfg: ComplexityConfig): number;
export function baselineAt(i: number, cfg: HazardBaselineConfig | RewardBaselineConfig): number;
```

`actualCeiling` draws jitter from `rng.fork(\`reach-entropy:${i}\`)` — from the **World rng**, and
only when a Reach is actually being realized (never from previews).

`length-policy.ts` — `WorldLengthPolicy`; `drawWorldLength(seed, policy): number | undefined`
from `rng.fork("world-length")`; `AreaCountConfig` + `drawAreaCount(...)` (seeded at the Reach
root fork, after `finalCeiling` is known, so `weights` can read it).

## Phase 3.2 — Modifiers & requests

`modifiers.ts` — `ReachModifierDef`, `DialPatch`, `ReachModifierPolicy` exactly per redesign 04;
`applyDialPatches(base, mods): EffectiveDials` (deltas only, never overwrite; multiplier
composition per the formula); `validateModifierChoice(policy, depth, chosen): void` (`GenError`
on: below `requiredRange.min`, above `.max`, `minDepth` violation, `excludesTags` conflict).

`reach-request.ts` — `ReachRequest` (exact shape from redesign 04); `requestIdentity(request):
string` — canonical hash: FNV-1a over
`reach${i}:mods[${sortedIds}]:econ[${stableJson(gadgetEconomyOverride ?? {})}]:pecon[${…}]:tpl[${templateId ?? "pool"}]`;
this string is also the Reach's **root fork label** (redesign 02 conventions table).

`portals.ts` — `ReachPortal` shape; `WorldComposer` keeps `portals: ReachPortal[]`; a forward
portal is appended automatically when a Reach with a `terminal` Region is realized
(`fromSpaceHint: "terminal"`, `toSpaceHint: "hub"`, `oneWay: false` default); `addPortal()` for
host extras (both endpoints must be realized ⇒ else `GenError`).

## Phase 3.3 — Previews & the virtual schedule

`preview.ts` — `previewReachEnvelope(index): ReachEnvelopePreview` (exact shape from redesign 04).
It recomputes `expectedCeiling` + modifier ranges **without any RNG draw**: ranges use the
deterministic jitter *bounds* (±`JITTER_FRAC`), min/max modifiers from `policy.poolAt(depth)`
sorted by patch magnitude. `plannedCapabilities` reads the virtual schedule (below) when bounded.

`virtual-schedule.ts` — the generic `computeVirtualSchedule<T>` exactly per redesign 04, from
`rng.fork(forkNamespace)` on a **fresh Rng seeded by the WorldSeed only** (never the live world
rng — purity). Deterministic, cached on first call per namespace.

## Phase 3.4 — `world-composer.ts`

```ts
export interface WorldConfig { lengthPolicy?: WorldLengthPolicy; geometry?: boolean; /* + registry-provided configs */ }
export interface ContentSelector {   // M04 replaces the default; M03 default = "verbatim" strategy
  select(ctx: SelectionContext): { items: Item[]; gateRules: Rule[]; puzzles: PuzzleInstance[] };
}
export function createWorld(registry: Registry, seed: string, config?: WorldConfig): WorldComposer;

class WorldComposer {
  requestReach(request: ReachRequest): ReachResult;      // synchronous; full master-sequence steps 1–7 (L2+ arrive in M05–M08)
  previewReachEnvelope(index: number): ReachEnvelopePreview;
  readonly portals: ReachPortal[];
  readonly requestLog: ReachRequestRecord[];
  readonly realized: Map<number, ReachResult>;
}
```

`requestReach` implements steps 1–7 of the corrected master sequence
([redesign 01](../redesign/01-architecture.md)): validate request (legal next slot:
`fromReachIndex` realized; slot not already realized) → resolve template → `finalCeiling` →
**selector.select(...)** → `interpretTemplate(template, selected, nudges, reachRng)` →
`validateGraph` → `assumedFill` → record into `requestLog` + `realized`, update carried
`startHeld` for the next Reach (fold placed grants + non-volatile flags), fire the final-sweep
flag into the selection context when `index === L − 1`. `ReachResult` for now =
`{ meta, graph, placement, relaxations }` (descriptors grow in M05–M08).

Until M04, ship `verbatimSelector(items, gateRules)` used by tests: it returns exactly what it
was constructed with (and honors the sweep flag by returning everything remaining).

## Phase 3.5 — Tests (`world/world.test.ts`, `world/preview.test.ts`)

- **Worked table**: assert `expectedCeiling`/`finalCeiling` reproduce redesign 04's table rows
  (±1 for jitter-dependent columns using a pinned seed).
- **Clamps**: 6 stacked max-risk modifiers never exceed `ABSOLUTE_HARD_MAX`; rolling-average
  ceiling across tiers is non-decreasing over 60 Reaches (never assert adjacent pairs).
- **Preview purity**: call `previewReachEnvelope(3)` five times, then `requestReach` slots 0–3;
  compare the entire world byte-wise (stable JSON of results) against a control world that never
  previewed — identical.
- **Request-log reproducibility**: build world A, issue 4 requests with mixed modifiers; build
  world B from the same seed replaying `A.requestLog`; stable-JSON of all `ReachResult`s
  identical.
- **Request validation**: out-of-order slot, unrealized `fromReachIndex`, illegal modifier depth,
  duplicate realization — each throws `GenError` with the offending field named.
- **Virtual schedule**: bounded world — schedule is pure (repeat-call identical), covers every
  pool entry, spreads across `0..L−1`; namespaces `gadgets`/`puzzles` are independent (editing
  one pool's entries leaves the other's schedule unchanged).
- **Length policy**: `{min:1,max:1}` ⇒ `L = 1` and `previewReachEnvelope(0).isDeclaredFinalReach`
  true; unbounded ⇒ `plannedCapabilities` absent.
- **Portals**: terminal auto-portal appears; `addPortal` to an unrealized Reach throws.

## Definition of Done

- [ ] All fork labels match the redesign 02 conventions table verbatim (grep-check them).
- [ ] Previews and schedules provably consume no live RNG (the purity tests above).
- [ ] `requestReach` is synchronous, pure given (world state, request), and appends exactly one
      `requestLog` record.
- [ ] Full root suite + typecheck green. STOP — hand off. Do not commit.

## Pitfalls

- The lookbehind term reads the *realized* previous ceiling — store it on the realized record,
  don't recompute.
- `previewReachEnvelope` must not touch `realized` entries beyond reading already-fixed facts —
  never trigger lazy computation from inside a preview.
