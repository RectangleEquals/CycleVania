/**
 * **What each surface owns.**
 *
 * ⚠ **A toolbar belongs to the active asset tab, not to the window — and so does the menu bar.**
 * Unreal's level tab reads `File Edit Window Tools Build Select Actor Help`; its Blueprint tab reads
 * `File Edit Asset View Debug Window Tools Help`. ▶ **The whole chrome changes**, which is why this is a
 * table of surfaces rather than one toolbar with things switched off.
 *
 * `10-editor.md` §9b.
 */

import { layerGroup, type ToolGroup } from "./frame.ts";

/** The chrome one surface asks for. */
export interface Surface {
  menus: string[];
  groups: ToolGroup[];
  /** The stage's own document tabs, if the surface has any. */
  documents?: { id: string; label: string }[];
}

/** ⚠ Present on every surface: save and browse-to. Unreal's two toolbars share these and little else. */
const PROJECT: ToolGroup = {
  id: "project",
  intent: 1,
  keep: 30,
  items: [
    { id: "save", label: "Save", hint: "Save the active asset" },
    { id: "saveall", label: "Save All" },
  ],
};

/** ⚠ **One menu, two doors** — the same categorized create menu the Content Browser opens. */
const CREATE: ToolGroup = {
  id: "create",
  intent: 3,
  keep: 20,
  items: [{ id: "add", label: "Add", menu: true, hint: "Create content" }],
};

/**
 * The docks group.
 *
 * ⚠ **These are toggles, not navigations.** §9d keeps them off at depth 0 — a developer who has not
 * asked for the trace has not met the concept, and a dark button they cannot explain is worse than
 * no button.
 */
const docksGroup = (shown: Record<string, boolean>): ToolGroup => ({
  id: "docks",
  intent: 4,
  keep: 10,
  items: [
    { id: "dock:outline", label: "Outline", toggle: true, on: !!shown.outline },
    { id: "dock:details", label: "Details", toggle: true, on: !!shown.details },
    { id: "dock:content", label: "Content", toggle: true, on: !!shown.content },
  ],
});

/**
 * ▶ **`Generate` is CycleVania's Play button**, and it gets Play's position and weight: far right,
 * alone, in its own group.
 */
const RUN: ToolGroup = {
  id: "run",
  intent: 5,
  keep: 100,
  items: [{ id: "generate", label: "Generate", hint: "Build a level from this project's content" }],
};

/**
 * The generated result — the leftmost tab, and the one that cannot be closed.
 *
 * ⚠ **Its verb is the layer switcher**, because looking at this project's output at five depths is what
 * this surface is *for*.
 */
export function resultSurface(reached: number, layer: string, docks: Record<string, boolean>): Surface {
  return {
    menus: ["File", "Edit", "Window", "Tools", "Help"],
    groups: [PROJECT, layerGroup(reached, layer), CREATE, docksGroup(docks), RUN],
  };
}

/**
 * An asset editor's surface.
 *
 * ⚠ **Deliberately thin until the editors exist.** M20e, M20f and M20g each own their surface's verb —
 * `Compile` on a schematic, the tangent modes on a curve — and inventing those here would be guessing
 * at work three milestones own. ▶ What M20b settles is that **the chrome swaps at all**.
 */
export function assetSurface(kind: string, docks: Record<string, boolean>): Surface {
  return {
    menus: ["File", "Edit", "Asset", "View", "Window", "Help"],
    groups: [
      PROJECT,
      { id: "verb", intent: 2, keep: 90, items: [{ id: "kind", label: kind, enabled: false, hint: `${kind} editor — M20e-M20g` }] },
      CREATE,
      docksGroup(docks),
      RUN,
    ],
  };
}
