# 01 · Determinism law

Everything a generated world depends on flows from a single seed through deterministic primitives. This
is what lets a server generate once and clients reconstruct identically (co-op), and what makes replays
and cross-platform parity possible.

## The rules

1. **No `Math.random`.** All randomness comes from `Rng` (`math/rng.ts`) — an sfc32 core with FNV-1a
   string seeding and splitmix32 expansion.
2. **No host trig for generation state.** Use `dsin/dcos/datan/datan2` (`math/trig.ts`) — minimax
   polynomials, pure arithmetic, identical on every JS engine (unlike `Math.sin`). `Math.floor/sqrt/imul`
   etc. are fine (they're deterministic).
3. **Fork, don't share.** `rng.fork(label)` derives an independent child stream **without advancing** the
   parent, so adding a draw in one subsystem never perturbs another. Seed sub-streams by **stable
   identity** (role/id/coord), not array index, so content edits don't reshuffle unrelated worlds.

```ts
const rng = new Rng(`${worldSeed}:reach${i}`);
const fill = rng.fork("fill");     // independent, stable
```

## Golden-vector parity

`math/golden/*.json` pins the exact outputs of the reference determinism math. The hermetic
`golden.test.ts` re-runs the ported code and asserts byte-for-byte equality. If it ever fails, generated
worlds would diverge (and co-op clients desync) — regenerate only via `node scripts/gen-golden.ts` after a
deliberate reference change.

## CI guard (recommended)

Grep the built source for `Math\.random` or `Math\.(sin|cos|tan|atan)` and fail the build on a hit — a
single leak silently desyncs multiplayer.
