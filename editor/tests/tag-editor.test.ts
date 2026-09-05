/**
 * **The tag editor.**
 *
 * ⚠ **`/Core/Tag` is *"a dotted hierarchical label, picked rather than typed"***, and the assertions
 * follow from that phrase: a vocabulary exists so a `Tag` field can be a picker, and the tree it
 * implies must distinguish what was *declared* from what merely exists as structure.
 */

import { describe, expect, it } from "vitest";
import { declared, drawTags, tagStyles, tagTree, type TagDef } from "../src/tag-editor.ts";

const TAGS: TagDef[] = [
  { name: "surface.stone", doc: "bare stone", uses: 4 },
  { name: "surface.stone.wet", doc: "slippery", uses: 0 },
  { name: "hazard.fire", uses: 2 },
];

describe("the tree a dotted vocabulary implies", () => {
  it("nests by segment", () => {
    const t = tagTree(TAGS);
    expect(t.map((n) => n.segment)).toEqual(["hazard", "surface"]);
    expect(t[1]!.children[0]!.path).toBe("surface.stone");
    expect(t[1]!.children[0]!.children[0]!.path).toBe("surface.stone.wet");
  });

  it("marks an intermediate segment as implied, not declared", () => {
    // ⚠ **Deleting a declared tag and pruning an implied one are different actions** with different
    // consequences, so the editor may not blur them.
    const t = tagTree(TAGS);
    expect(t.find((n) => n.segment === "hazard")!.implied).toBe(true);
    expect(t.find((n) => n.segment === "surface")!.implied).toBe(true);
    expect(t[1]!.children[0]!.implied).toBe(false);
  });

  it("does not invent a declaration for a segment nobody declared", () => {
    expect(declared(TAGS)).toEqual(["hazard.fire", "surface.stone", "surface.stone.wet"]);
  });

  it("survives a single-segment tag", () => {
    expect(tagTree([{ name: "solo" }])[0]!.implied).toBe(false);
  });
});

describe("drawing", () => {
  it("says which are implied", () => {
    expect(drawTags(TAGS)).toContain("implied");
  });

  it("marks an unused tag as a warning, not an error", () => {
    // ⚠ **A vocabulary may legitimately run ahead of the content.**
    expect(drawTags(TAGS)).toContain("cv-warn");
    expect(drawTags(TAGS)).toContain("0 uses");
  });

  it("counts what is declared, not what the tree contains", () => {
    // ⚠ Five nodes, three declarations.
    expect(drawTags(TAGS)).toContain("3 declared");
  });

  it("names what a tag set is for, in the empty state", () => {
    // ▶ §9d — the one place a new developer is guaranteed to be looking.
    expect(drawTags([])).toContain("picker");
  });
});

describe("the editor keeps the theme's promise", () => {
  it("takes every colour from the four parameters", () => {
    expect(tagStyles().replace(/var\(--cv-[a-z]+\)/g, "")).not.toMatch(/#[0-9a-fA-F]{3,6}/);
  });
});
