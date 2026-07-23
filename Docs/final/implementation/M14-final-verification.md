# M14 · Final verification & hardening

**Goal**: prove the whole system against the complete invariant inventory, establish the
performance benchmark, sweep for contract drift, and perform the final acceptance hand-off. This
milestone adds tests and fixes — no new features.

**Required reading**: [redesign 15 (all)](../redesign/15-verification-and-test-strategy.md);
[redesign 02 §What can legitimately change a world](../redesign/02-determinism.md).

**Prerequisites**: M12 and M13 green.

## Phase 14.1 — Invariant sweep

Walk **every row** of redesign 15's four invariant tables (determinism, solvability & logic,
pacing & scheduling, spatial & geometry, simulation) and record, in a new
`packages/core/src/invariants.audit.md` (a checked-in checklist, not a test), where each is
covered: `<invariant> → <test file>::<test name>`. Any row without coverage gets its test
written **now** in the owning module's suite. Rows most likely still missing at this point:

- Fork-independence (the dummy-fork perturbation test, redesign 15 determinism table).
- Callback purity double-run across all three presets.
- Rolling-average ceiling monotonicity over 60 Reaches on `prime`.
- Anchor overlap invariants on the `prime` preset specifically (densest data).
- The instrumented solvability-locality assertion (Reach *i* reads nothing from *i+1*): wrap the
  realized-map in a recording proxy during one test generation and assert access patterns.
- The three tooling-era rows: diagnostics purity (M08/M10 tests — verify both are mapped),
  progress monotonicity incl. slab-sliced parity (M10), and the bundle round-trip on **both**
  paths (CLI: M13's suite; Inspector: the M12 manual round-trip — write the missing automated
  Inspector-path test here if M12 left it manual-only).

## Phase 14.2 — The benchmark (the one big test)

`packages/examples/src/benchmark.test.ts`, marked with a generous explicit timeout (120 s) and
`describe` name `"benchmark (slow)"`:

- One seed × `prime` preset, 1 Reach, `geometry: true` at full preset dims.
- Assert: completes; per-Area finish time < 10 s each (measure with `performance.now` — allowed
  in tests); serialized descriptor < 25 MB; `pieces.length / instances.length < 0.5`; zero
  relaxation storms (< 5 total).
- Record the numbers into the test output (console.info) — these are the baselines future work
  is measured against; adjust the ceilings only with maintainer approval.

Also verify the *rest* of the suite's discipline: total root `pnpm test` runtime (excluding the
benchmark) under ~4 minutes on the dev machine; if over, find the offender (usually an
accidentally-geometry-on or too-deep fixture) and shrink it — do not raise timeouts.

## Phase 14.3 — Contract drift sweep

- **API surface**: diff `core/src/index.ts` exports against every shape named in redesign 04–12;
  missing exports get added; extra public exports get justified or made internal.
- **Dial audit**: for every row of redesign 14's dial table, assert (one big table-driven test)
  that the dial exists at the documented location with the documented default. Buried constants
  found along the way get promoted to dials or explicitly documented as non-dials with
  maintainer sign-off.
- **Fork-label audit**: grep all `\.fork\(` call sites; each label must match redesign 02's
  conventions table; update the table's "extended as modules are built" rows in a short
  addendum file `Docs/final/implementation/fork-labels.md` (the implementation suite may grow
  this file; the redesign doc stays untouched).
- **Error taxonomy**: every `throw` in `core/src` is a `GenError` subclass, `GenCancelled`, or a
  commented internal-bug `Error` — audit and fix.

## Phase 14.4 — Acceptance protocol

1. `pnpm -r typecheck` — green, one-shot.
2. `pnpm test` — full suite incl. soak matrix, green, one-shot; benchmark run once separately
   (`npx vitest run benchmark --testTimeout=180000` from root).
3. `pnpm cyclevania validate` + `soak --seeds 100` on all three presets — green.
4. `pnpm cyclevania report` on `classic`, one seed — open the report, verify the mermaid graph,
   sphere table, and dial snapshot render sensibly.
5. Inspector: build; one-shot screenshot pass over all views + Play mode (browser + server
   killed after); every screenshot actually viewed.
6. Write the hand-off summary: what exists, benchmark numbers, any deviations recorded in
   `invariants.audit.md`, open questions.
7. **STOP.** The maintainer reviews, tests by hand, and commits. Never commit.

## Definition of Done

- [ ] `invariants.audit.md` maps every redesign 15 row to a passing test.
- [ ] Benchmark green with recorded baselines; non-benchmark suite < ~4 min.
- [ ] Zero contract drift found-and-unfixed (API, dials, fork labels, errors).
- [ ] Acceptance protocol executed top to bottom in one sitting.
- [ ] Hand-off summary delivered. Do not commit.
