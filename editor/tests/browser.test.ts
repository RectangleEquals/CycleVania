/**
 * **The Content Browser** — one of it, and it is the drawer.
 *
 * ⚠ **The assertions are about what makes it *the* browser rather than a panel that lists files**: the
 * two populations under the content root, the tile that answers *"what is this"* without a hover, the
 * two-tier create menu, and the empty state that teaches.
 */

import { describe, expect, it } from "vitest";
import {
  CONTENT_KINDS,
  CREATE_MENU,
  IMPORTED_EXTS,
  browserStyles,
  contentAt,
  createMenu,
  crumbs,
  drawBrowser,
  folderTree,
  type BrowserState,
} from "../src/browser.ts";
import { ICON_FOR_EXT, LAYER_ICON, hasIcon, icon, iconForPath, iconNames } from "../src/icons.ts";

const FILES = [
  "schematics/Hookshot.cvs",
  "schematics/Plaque.cvs",
  "schematics/doors/IronDoor.cvs",
  "curves/progression.cvcurve",
  "progression/unlocks.cvunlock",
  "meshes/hookshot.glb",
];

const state = (over: Partial<BrowserState> = {}): BrowserState => ({
  kinds: [],
  folder: "",
  search: "",
  docked: true,
  ...over,
});

describe("browsing", () => {
  it("shows folders and files at the current level, not a flat list", () => {
    const { folders, files } = contentAt(FILES, state());
    expect(folders).toEqual(["curves", "meshes", "progression", "schematics"]);
    expect(files).toEqual([]);
  });

  it("descends into a folder", () => {
    const { folders, files } = contentAt(FILES, state({ folder: "schematics" }));
    expect(folders).toEqual(["doors"]);
    expect(files.map((f) => f.name)).toEqual(["Hookshot.cvs", "Plaque.cvs"]);
  });

  it("names a file's kind from its extension", () => {
    expect(contentAt(FILES, state({ folder: "curves" })).files[0]!.kind).toBe("Curve");
  });

  it("stacks filters, each toggled alone", () => {
    // ⚠ Unreal's filters are simultaneous — one active filter is a special case of several.
    expect(contentAt(FILES, state({ kinds: ["Schematic", "Curve"], folder: "schematics" })).files)
      .toHaveLength(2);
    expect(contentAt(FILES, state({ kinds: ["Curve"], folder: "schematics" })).files).toHaveLength(0);
  });

  it("searches within the current folder", () => {
    const { files } = contentAt(FILES, state({ folder: "schematics", search: "hook" }));
    expect(files.map((f) => f.name)).toEqual(["Hookshot.cvs"]);
  });

  it("gives a breadcrumb where every step back is a target", () => {
    expect(crumbs("schematics/doors")).toEqual([
      { label: "Content", path: "" },
      { label: "schematics", path: "schematics" },
      { label: "doors", path: "schematics/doors" },
    ]);
  });

  it("knows every content extension the design declares", () => {
    expect(CONTENT_KINDS.map((k) => k.ext).sort()).toEqual(
      ["cvcurve", "cvs", "cvspine", "cvstate", "cvtags", "cvunlock"].sort(),
    );
  });

  it("puts folders in the sources tree and files nowhere near it", () => {
    // ⚠ **Folders only** — a file in the tree makes the tree a second asset view.
    const tree = folderTree(FILES);
    expect(tree).toContain("schematics");
    expect(tree).toContain("schematics/doors");
    expect(tree.some((t) => t.endsWith(".cvs"))).toBe(false);
  });
});

describe("the content root holds two populations", () => {
  it("marks an imported asset as imported", () => {
    // ⚠ **`10-editor.md` §2.** A mesh has no editor and that is not a gap — it is authored in a DCC
    // tool, so the browser must not offer to open one.
    const { files } = contentAt(FILES, state({ folder: "meshes" }));
    expect(files[0]!.imported).toBe(true);
    expect(files[0]!.kind).toBe("Mesh");
  });

  it("does not mark an authored source as imported", () => {
    expect(contentAt(FILES, state({ folder: "curves" })).files[0]!.imported).toBe(false);
  });

  it("agrees with the icon table about what can be imported", () => {
    for (const ext of IMPORTED_EXTS) expect(ICON_FOR_EXT[ext]).toBe("mesh");
  });

  it("says in the tooltip that an imported asset opens outside the editor", () => {
    expect(drawBrowser(FILES, state({ folder: "meshes" }))).toContain("opens outside the editor");
  });
});

describe("tiles, not a list", () => {
  it("gives every tile a picture, a name and a kind", () => {
    // ▶ **Unreal's kind strip**: a tile answers *"what is this"* with no hover and no filter.
    const html = drawBrowser(FILES, state({ folder: "curves" }));
    expect(html).toContain("cv-tpic");
    expect(html).toContain("cv-tname");
    expect(html).toContain("cv-tkind");
  });

  it("draws the asset when it can, and falls back to the kind icon when it cannot", () => {
    // ⚠ **§9e: the kind icon is the fallback, not the plan.**
    const drawn = drawBrowser(FILES, state({ folder: "curves" }), {
      "curves/progression.cvcurve": "<svg id='real-thumb'></svg>",
    });
    expect(drawn).toContain("real-thumb");
    expect(drawBrowser(FILES, state({ folder: "curves" }))).toContain("<svg");
  });

  it("counts what is on screen", () => {
    expect(drawBrowser(FILES, state({ folder: "schematics" }))).toContain("3 items");
    expect(drawBrowser(FILES, state({ folder: "curves" }))).toContain("1 item<");
  });

  it("teaches in the empty state rather than saying nothing", () => {
    // ⚠ **The one place a new developer is guaranteed to be looking** — §9d.
    const html = drawBrowser([], state());
    expect(html).toContain("right-click to create content");
  });

  it("offers Dock in Layout, because the drawer and the dock are one surface", () => {
    expect(drawBrowser(FILES, state({ docked: false }))).toContain("Dock in Layout");
    expect(drawBrowser(FILES, state({ docked: true }))).toContain("Undock");
  });
});

describe("the create menu has two tiers", () => {
  it("keeps the basic list short", () => {
    // ⚠ **The split is the design.** A developer who wants the common thing never opens a submenu.
    expect(CREATE_MENU.basic.length).toBeLessThanOrEqual(4);
    expect(CREATE_MENU.basic.map((b) => b.id)).toEqual(["cvs", "cvcurve", "cvunlock"]);
  });

  it("puts everything else behind a category", () => {
    const advanced = CREATE_MENU.advanced.flatMap((g) => g.items.map((i) => i.id));
    expect(advanced).toContain("cvspine");
    expect(advanced).toContain("cvstate");
    expect(advanced).toContain("cvtags");
  });

  it("covers every content kind exactly once across both tiers", () => {
    // ⚠ A kind in neither tier cannot be created; a kind in both is two doors to one thing.
    const all = [
      ...CREATE_MENU.basic.map((b) => b.id),
      ...CREATE_MENU.advanced.flatMap((g) => g.items.map((i) => i.id)),
    ];
    expect([...all].sort()).toEqual(CONTENT_KINDS.map((k) => k.ext).sort());
  });

  it("gives every leaf an icon and a one-line description", () => {
    // ▶ **A menu of nouns with no descriptions is a quiz.**
    for (const e of [...CREATE_MENU.basic, ...CREATE_MENU.advanced.flatMap((g) => g.items)]) {
      expect(hasIcon(e.icon), e.id).toBe(true);
      expect(e.doc.length, e.id).toBeGreaterThan(10);
    }
  });

  it("says import is optional, right where a developer might assume otherwise", () => {
    // ⚠ **Create-from-scratch is the default** — `10-editor.md` §2 — and the menu must not imply a file
    // is needed.
    expect(createMenu()).toContain("can be created empty instead");
  });
});

describe("icons", () => {
  it("draws in currentColor, so the theme still owns every colour", () => {
    // ⚠ **§9a survives contact with iconography** — an icon with a baked hex is a second palette.
    for (const name of iconNames()) {
      const svg = icon(name);
      expect(svg, name).toContain("currentColor");
      expect(svg.replace(/currentColor/g, ""), name).not.toMatch(/#[0-9a-fA-F]{3,6}/);
    }
  });

  it("has one mark per content kind, and per layer", () => {
    for (const k of CONTENT_KINDS) expect(hasIcon(ICON_FOR_EXT[k.ext]!), k.ext).toBe(true);
    for (const id of ["L1", "L2", "L3", "L4", "L5"]) expect(hasIcon(LAYER_ICON[id]!), id).toBe(true);
  });

  it("gives the same mark to the same concept, wherever it appears", () => {
    // ⚠ **A concept with two marks has none.**
    expect(iconForPath("a/b/Hookshot.cvs")).toBe(icon("schematic"));
    expect(iconForPath("x.glb")).toBe(iconForPath("y.gltf"));
  });

  it("is inline SVG, never a font or a sheet", () => {
    // ⚠ **A font that fails to load leaves boxes.**
    expect(icon("schematic")).toMatch(/^<svg /);
    expect(browserStyles()).not.toMatch(/@font-face|url\(/);
  });

  it("hides itself from assistive tech, because the label beside it is the name", () => {
    expect(icon("save")).toContain('aria-hidden="true"');
  });

  it("returns nothing for a name it does not have, rather than a broken glyph", () => {
    expect(icon("no-such-icon")).toBe("");
  });
});

describe("the browser keeps the theme's promise", () => {
  it("takes every colour from the four parameters", () => {
    const bare = browserStyles().replace(/var\(--cv-[a-z]+\)/g, "");
    expect(bare.replace(/rgb\(0 0 0 \/ \.\d+\)/g, "")).not.toMatch(/#[0-9a-fA-F]{3,6}/);
  });
});
