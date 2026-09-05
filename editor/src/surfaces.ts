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
    { id: "save", label: "Save", icon: "save", hint: "Save the active asset" },
    { id: "saveall", label: "Save All", icon: "saveall", hint: "Save every unsaved asset" },
  ],
};

/** ⚠ **One menu, two doors** — the same categorized create menu the Content Browser opens. */
const CREATE: ToolGroup = {
  id: "create",
  intent: 3,
  keep: 20,
  items: [
    {
      id: "add",
      label: "Add",
      icon: "add",
      menu: true,
      // ⚠ **Not a parallel route.** It opens the same create menu the browser's right-click opens,
      // for when the drawer is closed — a convenience, and nothing the browser cannot also do.
      hint: "Create content — the same menu the Content Drawer opens",
    },
  ],
};

/*
 * ⚠ **The docks group was removed, and its absence is the point.**
 *
 * A first build put `Outline` / `Details` / `Content` toggles here. ▶ **A dock already carries its
 * own collapse, and `Window` restores a closed one** — so this was a second control for one state, and
 * the two drifted the first time anybody used either: collapsing Details left its toolbar toggle lit.
 * ⚠ **Two controls for one thing will disagree**, and the one further from the thing is the one that
 * lies. `10-editor.md` §9b.
 */

/**
 * ▶ **`Generate` is CycleVania's Play button**, and it gets Play's position and weight: far right,
 * alone, in its own group.
 */
const RUN: ToolGroup = {
  id: "run",
  intent: 5,
  keep: 100,
  items: [
    {
      id: "generate",
      label: "Generate",
      icon: "generate",
      hint: "Build a level from this project's content",
    },
  ],
};

/**
 * The generated result — the leftmost tab, and the one that cannot be closed.
 *
 * ⚠ **Its verb is the layer switcher**, because looking at this project's output at five depths is what
 * this surface is *for*.
 */
export function resultSurface(reached: number, layer: string): Surface {
  return {
    menus: ["File", "Edit", "Window", "Tools", "Help"],
    groups: [PROJECT, layerGroup(reached, layer), CREATE, RUN],
  };
}

/**
 * An asset editor's surface.
 *
 * ⚠ **Deliberately thin until the editors exist.** M20e, M20f and M20g each own their surface's verb —
 * `Compile` on a schematic, the tangent modes on a curve — and inventing those here would be guessing
 * at work three milestones own. ▶ What M20b settles is that **the chrome swaps at all**.
 */
export function assetSurface(kind: string, kindIcon: string, schematic = false): Surface {
  // ⚠ **A schematic's verb is `Compile`** — Unreal's Blueprint toolbar leads with it, and it is the
  // thing this surface exists to do. ▶ Everything else is still the shared chrome.
  const verb: ToolGroup = schematic
    ? {
        id: "verb",
        intent: 2,
        keep: 90,
        items: [
          { id: "compile", label: "Compile", icon: "compile", hint: "Compile this schematic" },
          { id: "findrefs", label: "Find", icon: "outline", hint: "Find references" },
        ],
      }
    : {
        id: "verb",
        intent: 2,
        keep: 90,
        items: [
          { id: "kind", label: kind, icon: kindIcon, enabled: false, hint: `${kind} editor — M20f-M20g` },
        ],
      };
  return {
    menus: ["File", "Edit", "Asset", "View", "Debug", "Window", "Help"],
    groups: [PROJECT, verb, CREATE, RUN],
    // ⚠ **One tab per opened hook or graph** — a developer working two hooks has two tabs.
    documents: schematic
      ? [
          { id: "objects", label: "Objects" },
          { id: "onpickup", label: "OnPickup" },
        ]
      : undefined,
  };
}
