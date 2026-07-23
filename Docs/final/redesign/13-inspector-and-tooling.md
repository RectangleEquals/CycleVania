# 13 · Inspector & tooling (the dataset workbench)

> The Inspector is not a debug viewer — it is the **primary authoring tool** for CycleVania
> datasets: create, modify, and test registries and dials visually, play-test the result like a
> player, and export a **reproduction bundle** that makes any displayed world exactly
> reproducible in a host project. A headless CLI covers the same ground for CI and scripting.
> Together they close goal 6 of [00](./00-goals-and-principles.md): *no guesswork, ever*.

## Separation of concerns

- `@cyclevania/inspector` — Vite + Three.js app. Imports **only the public core API** and
  `@cyclevania/examples` presets. Nothing in core knows the Inspector exists.
- `@cyclevania/cli` — Node CLI over the same public API. No rendering; emits JSON/markdown/SVG.
- Reference realizer — the Inspector's kit renderer doubles as the documented example realizer
  ([10](./10-output-contract.md)); hosts copy it as their starting point.

## The Inspector

### Layout

Left: navigator + data panels. Center: the 3D/graph viewport. Right: detail panel for the current
selection. Bottom: collapsible console (REPL). Top bar: seed, preset, mode, regenerate, export.

### Per-layer views (the drill-down)

Each pipeline layer gets a dedicated view of the *same* generated world, so a dev can see every
decision at the altitude it was made — this is the "auto-updating flow charts for generated
worlds, and every aspect they contain" requirement, made concrete:

| View | Shows | Interactions |
|---|---|---|
| **World** | realized Reaches as nodes, `ReachPortal`s as edges, per-slot envelope previews for unrealized slots | request a Reach (with a modifier-picker exercising the real policy), inspect the request log |
| **Mission (L1)** | the Reach's live mission graph as an auto-laid-out flowchart: Regions colored by role, edges labeled with their full Rules, flags with provenance, Locations with placed items + sphere badges; an animatable **sphere ladder** (step through Sphere 0..n and watch reachability flood) | select any node/edge → full detail (rule, `missingCaps` vs. a configurable Held); export this exact diagram (below) |
| **Schedule** | both pools' virtual schedules vs. actual placements across Reaches; eligibility curves per entry; pity/sweep events | hover a capability → its Facets, buckets, powerWeight curve |
| **Skeleton (L2)** | Spaces as envelope boxes at their laid-out positions; sockets as oriented arrows (provisional vs. resolved togglable); connector plans; degree/budget readouts; outdoor/landmark flags | drag nothing (layout is generated!) — select → why-this-position detail (forces, pins, z-plan) |
| **Volume (L3)** | the composed SDF: fast raymarched preview or a coarse preview mesh; biome gradient overlay; content anchors as glyphs by kind; landmark LOS rays | scrub the sub-biome blend; toggle anchor kinds |
| **Geometry (L4)** | the real generated kit, instanced and flat-shaded (facets visible); piece inspector (select an instance → its `PieceMeta`, its piece's usage count); occupancy-grid slice viewer; dressing anchors | x-ray cutaway; per-surface-kind color mode; budget meters (polys, unique pieces) |

All views stay in sync on one selection model (select a Region in L1 → its Area highlights in
L2–L4). Camera transitions tween (ease-out); single-click selects, double-click changes scope —
scope changes are never accidental.

### Play mode

The immersive test of what actual gameplay is expected to feel like — a "watered-down" but
faithful walk of the real world, driven entirely by the sim ([11](./11-simulation-and-autosolve.md)):

- First-person flycam **locked inside the world**: swept-sphere collision against the occupancy
  grid ([09](./09-naturalization-and-kit.md)) — you leave a Space only through real openings, by
  gameplay means. The camera's current Space is tracked at all times.
- **Cascading visibility**: visited Spaces + everything visible through currently-open links stay
  rendered; a gated door reveals the *next* Space but nothing beyond until visited; fully-open
  passages are peer-through/fly-through. Driven by `SimState.visited` + link openness.
- **Diegetic messages for every event**: pickups (with what they unlock), uses (with what
  changed), unlocks, transitions, and an on-entry "what you can see from here" that respects
  perception gates (no seeing behind a false wall without the perception capability; a visible
  high ledge without the means to know what's up there) — repeatable via `/see`.
- Sphere hints + remembered locks (full unmet sets via `missingCaps`) in the left panel; the
  console (collapsed by default) accepts every sim command; `/solve` animates the autosolve log.

### The data editor

Datasets are created and edited *in* the Inspector — schema-driven forms + JSON editing per
registry ([12](./12-orchestration-and-host-integration.md) table), with:

- **Live validation** — the same `defineRegistry` errors, shown inline at the offending entry
  (orphan tags, volatile-flag violations, bootstrap failures) before you ever generate.
- **Start empty or from a preset** ([14](./14-dial-reference-and-presets.md)); diff-against-preset
  highlighting so a host sees exactly what they've customized.
- **Dial panel** — every dial from [14](./14-dial-reference-and-presets.md) live-editable with its
  default, doc link, and current effective value (after modifier patches); regenerate on change.
- **Seed lab** — pin a seed, browse neighbors, A/B two seeds side-by-side (world diff: graph
  diff, placement diff, autosolve-log pacing diff).

### The diagnostics panel — warnings and errors, visually

The Inspector routes CycleVania's diagnostics channel ([12](./12-orchestration-and-host-integration.md))
into a dedicated, always-reachable panel: every warning and error from the last generation
(registry validation issues, recorded relaxations, budget pressure, res clamping), each with its
stable code, human message, and a **click-to-navigate path** — clicking `skeleton.junction-inserted
@ reach0/area:r2` selects that Area in the Skeleton view. Severity badges surface on the top bar
(a warnings count you can't miss), and the panel filters by level and by generation phase. The
verbosity dial (`DiagnosticsConfig.level`) is in the dial panel like everything else — turning it
to `debug`/`trace` is how a CycleVania developer watches the generator think without touching a
debugger.

### The default progress display

Any async generation the Inspector triggers (regenerate, a Reach request, a preset switch) shows
a **default progress overlay** built on the structured progress reports from
[12](./12-orchestration-and-host-integration.md): an overall bar (monotonic `fraction`), the
current phase label ("Meshing Area 3/5"), a per-phase sub-bar, elapsed time, and the derived
remaining-time estimate. It is deliberately a *reasonable default* — the same `GenProgress`
stream a host consumes for its own loading screen, rendered plainly — so what the Inspector
shows is exactly what a host can show.

### Reproduction bundles — the no-guesswork contract

One button: **Export bundle** — a single JSON file containing the registry (full data), every
dial value, the seed, the request log, and `generationVersion`/`registryFingerprint`. Importing a
bundle anywhere (Inspector, CLI, a host's `defineRegistry` + `createWorld`) reproduces the
displayed world byte-identically — the fingerprint check tells you immediately if it can't and
why ([02](./02-determinism.md), "what can legitimately change a world"). Bundles are the unit of
sharing between designers, bug reports, and host integration: "use this bundle" replaces every
"what settings were you on?" conversation.

### Import is a first-class peer of export

Everything exportable is importable — **from actual files, not just a preset menu**. The
Inspector accepts, via both a file picker and viewport drag-and-drop:

- **Full reproduction bundles** — load registry + dials + seed + request log, regenerate, and
  show the "reproduction verified" badge (or the explanatory fingerprint-mismatch panel: what
  differs — data, callbacks/revision, or engine version — and what that means, per
  [02](./02-determinism.md)).
- **Standalone dataset files** — a registry (or any single catalog/config slice: just a gadget
  catalog, just a biome pack, just a dial snapshot) exported from the data editor. Partial
  imports merge into the current working dataset with a preview diff ("this replaces your
  puzzle catalog, 14 entries → 21") and run live validation before applying — an import that
  would produce a `GenError` is shown as errors-in-place, never half-applied.
- **World descriptors** — a previously generated world JSON opens read-only in every view (no
  regeneration needed), which is how a collaborator inspects a bug report's world without the
  producing dataset.

The CLI mirrors this: every `<bundle>` argument accepts a bundle **file path** (or a
module-reference bundle for callback-bearing registries), and `validate`/`report`/`diff` accept
standalone dataset and descriptor files too. Export and import are symmetric round-trips by
test: export → import → regenerate must be byte-identical.

## The CLI (`@cyclevania/cli`)

For CI, scripting, and hosts who never open the Inspector:

| Command | Does |
|---|---|
| `cyclevania generate <bundle\|registry> --seed S [--reaches N] [--geometry]` | emit descriptors (JSON) |
| `cyclevania validate <registry>` | run `defineRegistry` + template validation; exit code + diagnostics |
| `cyclevania soak <bundle> --seeds N` | solvability + autosolve + invariant soak; summary table |
| `cyclevania report <bundle> --seed S -o world-report.md` | the **world report**: an auto-generated markdown/HTML document with the mission-graph flowchart (mermaid), sphere table, placement table, dial snapshot, schedule vs. plan chart, geometry budget stats — the shareable, doc-ready description of one world |
| `cyclevania diff <bundleA> <bundleB>` | world/graph/placement/pacing diff of two bundles or seeds |
| `cyclevania export-diagram <bundle> --seed S --view mission` | just the diagram (mermaid/SVG) — the same source the Inspector's L1 view renders, so docs and tool never drift |

The report and diagrams are generated **from descriptors**, not from a parallel model — whatever
ships in output is what gets drawn, so visuals can never lie.

Every subcommand takes `--log-level <error|warn|info|debug|trace>` (default `warn`), routing the
same diagnostics channel the Inspector displays to stderr with stable codes — grep-able CI
output and the CycleVania-debugging workflow in one flag. Long-running commands (`generate
--geometry`, `soak`) render the `GenProgress` stream as a terminal progress line.

## Verification workflow note (for CycleVania's own development)

Inspector acceptance runs are one-shot: build, screenshot via a single headless-browser launch
with a temp profile, view the screenshot, and kill the preview server and browser immediately —
never leave either running. The CLI's `soak`/`validate` are the CI gate; the Inspector is the
human gate.
