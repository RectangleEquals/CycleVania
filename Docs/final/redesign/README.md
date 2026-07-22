# CycleVania — Final Redesign

This folder is the **single authoritative design** for CycleVania. It supersedes `Docs/*.md`
(original design), `Docs/dev/` (Phase D dev notes), and `Docs/redesign/` (clean-sheet proposal) —
all of which are retained as prior art only. Where any older document disagrees with this folder,
**this folder wins**.

The suite is written to two audiences at once: a developer new to procedural generation (every
algorithm is explained from first principles, with flowcharts), and an implementing agent that must
build the entire system from these documents alone, with no other context, missing nothing.

## Reading order

| Doc | Contents |
|---|---|
| [00 · Goals & principles](./00-goals-and-principles.md) | What CycleVania is, the six product goals, the fidelity spectrum, non-negotiables, lineage, glossary |
| [01 · Architecture](./01-architecture.md) | The five layers, composer hierarchy, separation of concerns, master order of operations, module layout |
| [02 · Determinism](./02-determinism.md) | The determinism law, RNG/trig/noise primitives, fork rules, reproducibility unit, golden vectors |
| [03 · Mission graph (L1)](./03-mission-graph.md) | Rules, Regions, Spheres, assumed fill, templates, per-Reach solvability |
| [04 · Worlds, Reaches & pacing](./04-worlds-reaches-and-pacing.md) | On-demand ReachRequests, modifiers, the complexity formula, virtual schedules, ReachPortals |
| [05 · Capabilities & Facets](./05-capabilities-and-facets.md) | The game-agnostic capability contract, scheduling, the gadget economy, world-shaping feedback |
| [06 · Puzzles, Locks & recipes](./06-puzzles-locks-and-recipes.md) | The first-class Puzzle pool, the lock taxonomy, spatial recipes |
| [07 · Spatial skeleton (L2)](./07-spatial-skeleton.md) | AreaComposer, force-directed layout, sockets & signatures, connectivity degrees |
| [08 · Volumetric composition (L3)](./08-volumetric-composition.md) | SDF hulls, outdoor spaces, spline connectors, socket resolution, content anchors, biomes, landmarks |
| [09 · Naturalization & the generated kit (L4)](./09-naturalization-and-kit.md) | Fidelity profiles, dual contouring, kit dedup, occupancy & collision, dressing |
| [10 · Output contract](./10-output-contract.md) | Every descriptor shape, serialization, the realizer guide, streaming & multiplayer |
| [11 · Simulation & autosolve](./11-simulation-and-autosolve.md) | The deterministic playtest reducer and the bot-completes-a-Reach proof |
| [12 · Orchestration & host integration](./12-orchestration-and-host-integration.md) | Async facade, horizon prefetch, workers, errors, the host checklist |
| [13 · Inspector & tooling](./13-inspector-and-tooling.md) | The dataset workbench, per-layer views, Play mode, reproduction bundles, the CLI |
| [14 · Dial reference & presets](./14-dial-reference-and-presets.md) | Every host-facing dial in one table, calibration philosophy, shipped presets |
| [15 · Verification & test strategy](./15-verification-and-test-strategy.md) | Every invariant, soak suites, fixture datasets, performance discipline |

An implementation-plan suite will follow separately (in `Docs/final/implementation`) once this
design is reviewed and approved. These documents deliberately contain design + algorithms + shapes
+ acceptance criteria, but no build sequencing beyond a suggested order in
[01](./01-architecture.md).
