/**
 * **The shell's frame.**
 *
 * ⚠ **The assertions are about the rules that make the arrangement *mean* something**, not that markup
 * came out. A frame renders whatever you give it; what is worth checking is that the three tab strips
 * stay distinct, that a narrowing toolbar drops the right things, that a layer nobody has generated is
 * *disabled rather than absent*, and that startup shows nothing a newcomer has not met.
 */

import { describe, expect, it } from "vitest";
import {
  LAYERS,
  assetTabs,
  dock,
  documentTabs,
  fitToolbar,
  frameStyles,
  groupWidth,
  layerGroup,
  menuBar,
  statusBar,
  toolbar,
  type Dock,
  type ToolGroup,
} from "../src/frame.ts";

const group = (id: string, intent: 1 | 2 | 3 | 4 | 5, keep: number, n = 2): ToolGroup => ({
  id,
  intent,
  keep,
  items: Array.from({ length: n }, (_, i) => ({ id: `${id}${i}`, label: `${id}${i}` })),
});

const GROUPS: ToolGroup[] = [
  group("project", 1, 20),
  group("verb", 2, 90),
  group("create", 3, 40),
  group("docks", 4, 10),
  { id: "run", intent: 5, keep: 100, items: [{ id: "gen", label: "Generate" }] },
];

describe("P01 — three tab strips, and they are different things", () => {
  it("keeps the generated result unclosable", () => {
    // ⚠ **The stage never empties.** A closable everything leaves a developer with a blank window and
    // no way back to the thing the project is *for*.
    const html = assetTabs(
      [
        { id: "result", label: "ForestTemple", fixed: true },
        { id: "hook", label: "Hookshot.cvs", dirty: true },
      ],
      "result",
    );
    expect(html).not.toContain(`data-close="result"`);
    expect(html).toContain(`data-close="hook"`);
  });

  it("marks unsaved on the tab, not in a dialog", () => {
    expect(assetTabs([{ id: "a", label: "A", dirty: true }], "a")).toContain("cv-dirty");
  });

  it("right-aligns the active asset's defining fact", () => {
    // ▶ Unreal puts `Parent class: Actor` there — the one thing about the asset that frames everything
    // else in the window.
    const html = assetTabs(
      [
        { id: "a", label: "A", fact: "Class: Door" },
        { id: "b", label: "B", fact: "Class: Lamp" },
      ],
      "b",
    );
    expect(html).toContain("Class: Lamp");
    expect(html).not.toContain("Class: Door");
  });

  it("draws no document strip when the surface has no documents", () => {
    // ⚠ An empty strip is a promise of tabs that do not exist.
    expect(documentTabs([], "x")).toBe("");
  });

  it("draws the document strip separately from the asset strip", () => {
    const docs = documentTabs([{ id: "og", label: "OnPickup" }], "og");
    expect(docs).toContain("cv-doctabs");
    expect(docs).not.toContain("cv-assettabs");
  });
});

describe("P02b — the toolbar survives a narrow window", () => {
  it("draws everything when there is room", () => {
    const { shown, overflow } = fitToolbar(GROUPS, 4000);
    expect(shown).toHaveLength(5);
    expect(overflow).toHaveLength(0);
  });

  it("keeps the verb and the run group longest", () => {
    // ⚠ **The claim the rule exists for.** `Generate` is this editor's Play button; losing it to a
    // narrow window would be losing the product.
    const total = GROUPS.reduce((w, g) => w + groupWidth(g), 0);
    const { shown } = fitToolbar(GROUPS, total / 2);
    const ids = shown.map((g) => g.id);
    expect(ids).toContain("run");
    expect(ids).toContain("verb");
    expect(ids).not.toContain("docks");
  });

  it("drops whole groups, never half of one", () => {
    // ⚠ A half-drawn group reads as a rendering bug, which is worse than an honest overflow.
    const { shown, overflow } = fitToolbar(GROUPS, 200);
    for (const g of [...shown, ...overflow]) {
      const original = GROUPS.find((o) => o.id === g.id)!;
      expect(g.items).toHaveLength(original.items.length);
    }
  });

  it("loses nothing — every group is either shown or in the overflow", () => {
    const { shown, overflow } = fitToolbar(GROUPS, 200);
    expect([...shown, ...overflow].map((g) => g.id).sort()).toEqual(
      GROUPS.map((g) => g.id).sort(),
    );
  });

  it("keeps the survivors in intent order, not in drop order", () => {
    const { shown } = fitToolbar(GROUPS, groupWidth(GROUPS[1]!) + groupWidth(GROUPS[4]!) + 40);
    expect(shown.map((g) => g.intent)).toEqual([...shown.map((g) => g.intent)].sort());
  });

  it("says how many groups it hid, rather than clipping silently", () => {
    // ⚠ **A toolbar that clips silently makes a missing button look like a missing feature.**
    const html = toolbar(GROUPS, 200);
    expect(html).toContain("cv-more");
    expect(html).toMatch(/title="\d+ more group/);
  });

  it("pushes the run group to the right", () => {
    const html = toolbar(GROUPS, 4000);
    expect(html.indexOf("cv-tspacer")).toBeLessThan(html.indexOf(`data-group="run"`));
  });
});

describe("P03 — the layer switcher", () => {
  it("offers the five layers the pipeline has", () => {
    // ⚠ Six layers, L0-L5; L0 is content, which is a dock and not a stage view.
    expect(LAYERS.map((l) => l.id)).toEqual(["L1", "L2", "L3", "L4", "L5"]);
  });

  it("disables a layer nobody has generated, rather than hiding it", () => {
    // ⚠ **"You have not run this far" and "this does not exist" are different answers**, and a missing
    // button gives the wrong one.
    const g = layerGroup(2, "L1");
    expect(g.items).toHaveLength(5);
    expect(g.items[1]!.enabled).toBe(true);
    expect(g.items[2]!.enabled).toBe(false);
  });

  it("says why a disabled layer is disabled", () => {
    // ⚠ A disabled control states its own precondition — `10-editor.md` §9b.
    expect(layerGroup(1, "L1").items[3]!.hint).toContain("not generated yet");
  });

  it("shows which layer is live, and only one", () => {
    const on = layerGroup(5, "L3").items.filter((i) => i.on);
    expect(on.map((i) => i.id)).toEqual(["L3"]);
  });

  it("marks nothing live when nothing has been generated", () => {
    // ⚠ **Found by looking.** At startup L1 rendered lit *and* disabled — two contradictory claims,
    // and the highlight is the one a developer believes.
    expect(layerGroup(0, "L1").items.some((i) => i.on)).toBe(false);
  });

  it("never marks a layer live that it also disables", () => {
    for (const reached of [0, 1, 3, 5]) {
      for (const item of layerGroup(reached, "L3").items) {
        expect(item.on && item.enabled === false, `${reached}/${item.id}`).toBe(false);
      }
    }
  });

  it("renders a disabled layer as disabled markup", () => {
    expect(toolbar([layerGroup(1, "L1")], 4000)).toContain("disabled");
  });
});

describe("P05 — docks are places, even closed", () => {
  const base: Dock = { id: "details", label: "Details", side: "right", present: true, body: "x" };

  it("is not drawn at all when it has nothing to say", () => {
    // ⚠ **§9d's rule.** An empty panel is a promise of work a developer has not asked for.
    expect(dock({ ...base, present: false })).toBe("");
  });

  it("collapses to a labelled strip rather than vanishing", () => {
    // ▶ A closed dock is still a *place*, which is what makes it findable again.
    const html = dock({ ...base, collapsed: true });
    expect(html).toContain("cv-dstrip");
    expect(html).toContain("Details");
    expect(html).toContain(`data-expand="details"`);
  });

  it("offers a way to fold it when open", () => {
    expect(dock(base)).toContain(`data-collapse="details"`);
  });

  it("keeps its side, so a restore knows where it came from", () => {
    expect(dock({ ...base, side: "bottom" })).toContain("cv-bottom");
  });
});

describe("P06 — the status bar carries what is true of the whole project", () => {
  it("shows the fingerprint, and explains what it answers", () => {
    // ⚠ **The fingerprint has no other home** — in a panel it looks like a property of something.
    const html = statusBar({ fingerprint: "8f2a1c", seed: 41, unsaved: 0 });
    expect(html).toContain("fingerprint 8f2a1c");
    expect(html).toMatch(/title="[^"]*identity/);
  });

  it("keeps seed separate from fingerprint, because they are orthogonal", () => {
    // ⚠ A different seed is the *same recipe*; a different fingerprint is a different build.
    const html = statusBar({ fingerprint: "8f2a1c", seed: 41, unsaved: 0 });
    expect(html).toContain("seed 41");
    expect(html).toMatch(/title="[^"]*same recipe/);
  });

  it("says so plainly when there is no seed yet", () => {
    expect(statusBar({ fingerprint: "—", seed: null, unsaved: 0 })).toContain("seed —");
  });

  it("says all saved rather than showing a zero", () => {
    // ⚠ `0 unsaved` is a count where a state was wanted.
    expect(statusBar({ fingerprint: "x", seed: 1, unsaved: 0 })).toContain("all saved");
    expect(statusBar({ fingerprint: "x", seed: 1, unsaved: 3 })).toContain("3 unsaved");
  });

  it("marks unsaved work as a warning, not as chrome", () => {
    expect(statusBar({ fingerprint: "x", seed: 1, unsaved: 3 })).toContain("cv-warn");
  });
});

describe("the frame keeps the theme's promise", () => {
  it("takes every colour from the four parameters", () => {
    // ⚠ **A component computing its own colour is a second theme**, and the second one goes stale the
    // first time the first one moves.
    const bare = frameStyles().replace(/var\(--cv-[a-z]+\)/g, "");
    expect(bare).not.toMatch(/#[0-9a-fA-F]{3,6}/);
  });

  it("answers hover, focus and disabled on every control it draws", () => {
    const css = frameStyles();
    for (const state of [":hover", ":focus-visible", ":disabled", ".is-on"]) {
      expect(css, state).toContain(state);
    }
  });

  it("gives the menu bar the same treatment as the toolbar", () => {
    // ⚠ The menu bar belongs to the active tab too — it is chrome, not a fixture.
    expect(menuBar(["File", "Edit"])).toContain(`data-menu="File"`);
  });
});
