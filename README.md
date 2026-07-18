# CycleVania

A standalone, **deterministic**, **renderer-free**, **data-driven** procedural-generation library
for 3D Metroidvanias. Utilizes cyclic dungeon generation + "sockets" + Archipelago-style solvability.

You supply *content* (item catalogs, lock vocabularies, a modular GeometryKit, grid resolutions,
macro-templates, complexity curves) as plain data; CycleVania runs the algorithms and returns structured,
**guaranteed-solvable** world data as abstract cell/kit assignments + socket transforms + connector plans.
Your game stitches and renders it. Nothing here imports a game engine, the DOM, or Node APIs.

```
World ─▶ Reach ─▶ Area (a 3D grid of room-nodes) ─▶ Room (a subdivided cluster of cells) ─▶ Cell
              └── connections            └── connectors (curved/45°/open/vertical)   └── kit piece + metadata + sockets + contents
```

## Packages (pnpm monorepo)

| Package | What |
|---|---|
| `@cyclevania/core` | Zero-dependency, renderer-free, deterministic procgen. **The library.** |
| `@cyclevania/examples` | Example data: the 24-gadget catalog, lock catalog, demo GeometryKit/template/registry + soak tests. |
| `@cyclevania/inspector` | Dev-only 3D inspector + interactive playtest simulator (Three.js + Vite). Not part of the published core. |

## Quickstart

```ts
import { composeReach, defineRegistry, buildSimWorld, autosolve } from "@cyclevania/core";

const registry = defineRegistry({
  grid: { areaCellSize: 16, roomCellSize: 2, snap: "ps2" },
  geometryKit: myGeometryKit,
  items: { catalog: [{ id: "skyhook", class: "progression", grants: "grapple",
                       profile: { grants: { gapSpan: 4 }, bias: { loopWeight: 0.2 } } }], startCaps: [] },
  locks: { chasm: { solvedBy: (r) => r.have("grapple"), recipe: "gap-crossing" } },
  styles: { "sunken-parish": { id: "sunken-parish" } },
});

const { reach, descriptor } = composeReach({ registry, seed: "expedition-7" }, {
  template: myReachTemplate, reachIndex: 3,
});

// realize `descriptor` into meshes in your engine; verify solvability with the simulator:
const world = buildSimWorld({ reach, descriptor }, registry);
autosolve(world); // a bot completes the Reach — softlock-free by construction
```

Or just use the examples:

```ts
import { demoReach, demoWorld } from "@cyclevania/examples";
const { descriptor } = demoReach("my-seed", 4);
```

## Commands (from the repo root)

```
pnpm install         # sets up the workspace
pnpm typecheck       # strict TS across all packages
pnpm test            # vitest: core unit + golden parity + examples solvability soak
pnpm inspector       # launch the 3D inspector (Vite dev server) → open in Chrome
```

## Guarantees

- **Deterministic**: seeded forkable RNG + minimax trig only — no `Math.random`, no host trig for
  generation state. Same seed ⇒ identical worlds on every JS engine (golden-vector parity tested).
- **Zero softlocks by construction**: Archipelago-style assumed-fill + an independent reachability
  regression check. The 1000-seed soak in CI has zero solver failures.
- **Renderer-free core**: abstract descriptors, never meshes. Your realizer owns geometry/poly budgets.

See [`Docs/`](./Docs) for the full guide.
