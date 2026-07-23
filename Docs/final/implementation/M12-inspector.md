# M12 · The Inspector

**Goal**: `@cyclevania/inspector` — the dataset workbench per redesign 13: per-layer synced
views, real-geometry rendering, full Play mode, the data editor with live validation, the dial
panel, seed lab, and reproduction-bundle export/import. This is the largest milestone; it is
split into four phases that each end shippable and screenshot-verified.

**Required reading**: [redesign 13 (all)](../redesign/13-inspector-and-tooling.md);
[redesign 10 §The realizer guide](../redesign/10-output-contract.md);
[redesign 11](../redesign/11-simulation-and-autosolve.md) (Play mode is a sim front-end).

**Prerequisites**: M11 green. Phase 12.4 additionally requires the reproduction-bundle shapes
from M13 Phase 13.2 (`core/src/descriptors/bundle.ts`) — execute that phase first if building
M12 before M13.

**Porting references**: legacy `packages/inspector/src/{main,scene,roomgraph}.ts` — the
scope/selection model, camera tweening, x-ray cutaway, and Play-mode UX logic contain many
hard-won interaction decisions; mine them freely, but the view structure is new (per-layer, not
per-container).

## Global structure

`packages/inspector/`: Vite + Three.js app (`three` + `vite` devDeps; deps `@cyclevania/core`,
`@cyclevania/examples`). `src/` layout: `main.ts` (boot + state), `state.ts` (one store:
settings, world, selection, mode), `views/{world,mission,schedule,skeleton,volume,geometry}.ts`,
`play/{play.ts, visibility.ts, messages.ts}`,
`panels/{navigator,detail,dials,editor,seedlab,progress,diagnostics}.ts`,
`realize/{kit-renderer.ts, materials.ts}` (**the reference realizer** — keep it exemplary and
commented; hosts copy it), `bundle.ts` (import/export via `core`'s bundle shapes + a
preset-picker fallback for callback data), `console.ts` (REPL over `parseCommand`/`step`).

Interaction invariants (from the legacy UX decisions — binding): single-click = select (never
moves scope), double-click = scope change (inspect) / interact (Play); every camera transition
tweens with ease-out; one selection model shared by all views (selecting a Region highlights its
Area/Spaces everywhere).

## Phase 12.1 — Scaffold + World/Mission/Schedule views

- Boot with `classicPreset`, seed `"cyclevania-demo"`, 1 realized Reach, geometry **on** at
  small dims.
- **World view**: realized Reaches as nodes, ReachPortals as edges, unrealized next slots as
  ghost nodes showing `previewReachEnvelope` numbers; a request dialog exercising the real
  modifier policy (pool at depth, requiredRange enforcement) → `requestReach`.
- **Mission view**: render the mermaid source from `core`'s `descriptors/diagram.ts` (bundle a
  mermaid renderer as a devDep, or lay out with a simple built-in DAG layout if lighter —
  either is acceptable; the *source* must come from the shared generator). Sphere ladder:
  a slider stepping Sphere 0..n recoloring reachable Regions/Locations from `ReachMeta.spheres`.
  Click any node/edge → detail panel (full rule, `missingCaps` vs. a configurable Held).
- **Schedule view**: two tracks (gadgets/puzzles) plotting virtual-plan slot vs. actual placed
  Reach per entry; hover → powerWeight curve + Facets summary.
- Left panel navigator + right detail panel + top bar (seed, preset, regenerate, export) exist
  from this phase.
- **Default progress overlay** (from this phase — every later phase regenerates through it): all
  generation runs through `requestReachAsync`; the overlay renders the `GenProgress` stream
  plainly per redesign 13 — overall bar, phase label, per-phase sub-bar, elapsed + derived
  remaining time. Keep it a self-contained component (`panels/progress.ts`) so it doubles as the
  documented host example.
- **Diagnostics panel** (`panels/diagnostics.ts`, from this phase): a `MemorySink` per
  generation run feeds a filterable list (level, phase) of every event — stable code, message,
  and a click-to-navigate `path` (selects the named Reach/Area/Space in the current view once
  those views exist; until then, selects in the navigator). Warning/error counts badge the top
  bar. The verbosity level is exposed in the dial panel (Phase 12.4) and as a top-bar quick
  toggle now.

Verification: build + one-shot screenshot of each of the three views **plus one showing the
progress overlay mid-generation and one of the diagnostics panel with a warning present** (use a
junction-forcing seed), viewed; servers/browser killed.

## Phase 12.2 — Skeleton/Volume/Geometry views

- **Skeleton view**: Space envelopes as translucent boxes at laid-out origins; sockets as
  oriented arrows (toggle provisional/resolved); connector splines as polylines; badges for
  outdoor/landmark/junction; per-Space budget/degree readout; the "why this position" detail
  (dials + recorded relaxations from meta).
- **Volume view**: coarse preview mesh of the composed field (run the mesher at 2× coarser res —
  reuse `finish/mesher` with a throwaway profile; never confuse it with the real kit); biome
  gradient overlay (vertex colors by blend weight); anchors as kind-glyph sprites with
  visibility toggles per kind; landmark LOS rays.
- **Geometry view**: the real kit via the reference realizer — one `InstancedMesh` per pieceId,
  flat shading, materials from `materialHint` + biome palette; piece inspector (click an
  instance → `PieceMeta`, piece usage count, highlight all instances of the piece); occupancy
  slice viewer (a draggable Z-plane rendering solid cells); dressing anchor toggle; budget
  meters (tris, unique pieces vs. limits); x-ray cutaway (port the legacy near-camera fade).

Verification: one-shot screenshots ×3, viewed.

## Phase 12.3 — Play mode

A sim front-end plus occupancy-grid collision — every rule below is binding
(they encode the maintainer's explicit UX feedback):

- Enter Play → spawn inside the entry Space at a floor anchor; first-person flycam;
  `collideSphere` against the occupancy grid every frame (radius ~0.4 cells) — the camera can
  leave a Space only through real openings.
- Track the camera's current Space continuously (containing-Space lookup by occupancy + Space
  bounds); it drives `SimState.at` (entering a Space applies `move` semantics: gate checks apply
  at the *aperture* — a gated socket's aperture is impassable (treat its cells as solid) until
  the gate passes via double-click interact).
- **Cascading visibility** from `SimState.visited` + link openness (redesign 13): visited Spaces
  + Spaces visible through currently-open links rendered; a gated door reveals the next Space
  only; recompute on every transition/unlock.
- **Diegetic messages** for every event (pickup with what it unlocks, use, unlock, transition,
  and the on-entry `see` report — perception-gated) as viewport toasts; console (collapsed by
  default) accepts all sim commands incl. `/see`, `/why`, `/solve` (animates the autosolve log
  step-by-step).
- Left panel in Play: sphere hints + remembered locks (every *seen* gate with its full
  `missingCaps` set).
- Selection in Play: single-click selects (no camera move); sidebar lists select-only;
  double-click interacts with the selected in-Space target.

Verification: one-shot screenshot inside a room with a toast visible + one of a gated door
detail; viewed; then a manual scripted checklist run (walk into a wall — blocked; pick up a
gadget — toast + panel update; unlock a door — next Space appears).

## Phase 12.4 — Data editor, dial panel, seed lab, bundles

- **Data editor**: per-registry schema-driven forms (generate the form from the shapes — a
  hand-maintained schema table in `editor.ts` is acceptable) + raw-JSON mode; **live
  validation** by re-running `defineRegistry` on change, errors inline at the entry;
  start-empty and start-from-preset; diff-against-preset highlighting.
- **Dial panel**: every dial from redesign 14's table, grouped by owner, showing default +
  current + effective (post-modifier) values, doc anchor links; edit → regenerate.
- **Seed lab**: pin a seed; neighbor browse (seed±1 shortcuts); A/B split view of two seeds with
  a graph/placement diff (reuse the CLI's diff logic — import from core-placed diff helpers if
  extracted, else duplicate minimally with a TODO).
- **Bundles & file import** (import is a first-class peer of export — redesign 13 §Import):
  export produces real files (registry-ref + overrides + dials + seed + request log +
  fingerprints); import accepts real files via **both a file picker and viewport drag-and-drop**,
  for three payload kinds, sniffed by shape:
  1. *Reproduction bundles* — load, fingerprint-check, regenerate; show "reproduction verified"
     on `stableStringify` equality, or the explanatory mismatch panel (what differs: data /
     callback revision / engine version) on failure.
  2. *Standalone dataset files* — a full registry or any single catalog/config slice exported
     from the data editor; **partial imports merge** into the working dataset behind a preview
     diff ("replaces your puzzle catalog, 14 → 21 entries") and live `defineRegistry`
     validation — a merge that would error shows the errors in place and applies nothing.
  3. *World descriptors* — open read-only in every view (no regeneration, no dataset needed) for
     inspecting a collaborator's bug-report world.

Verification: one-shot screenshots (editor with a validation error visible; dial panel; A/B
lab; **the partial-import preview diff**); viewed. Manual round-trips performed: bundle
export→import→verified badge; dataset slice export→drag-drop→merge; a descriptor JSON opened
read-only.

## Definition of Done

- [ ] All four phases screenshot-verified (screenshots actually viewed) with servers/browsers
      killed after each — no background processes left.
- [ ] The inspector imports only the public core/examples APIs (no deep imports into
      `core/src/*` internals except the documented public barrels).
- [ ] `pnpm --filter @cyclevania/inspector build` green; root typecheck + tests green.
- [ ] The reference realizer (`realize/kit-renderer.ts`) is clean and commented enough to serve
      as the documented host example.
- [ ] STOP — hand off. Do not commit.

## Pitfalls

- Never mutate descriptors for view state — keep all view/selection state in `state.ts`.
- Dispose Three.js geometries/materials on regenerate — the legacy inspector leaked on repeated
  regeneration; add a `disposeAll()` sweep.
- The Play-mode gate-as-solid trick needs the aperture's occupancy cells indexed per socket at
  load — precompute a socket→cells map from the aperture carve radius, don't scan per frame.
