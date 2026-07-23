import { describe, it, expect } from "vitest";
import type { CapabilityId } from "../logic/index.js";
import { buildHeld, aggregateBuckets, activeTags } from "./facets.js";
import type { CapabilityDef } from "./capability-def.js";

const defsOf = (list: CapabilityDef[]): Map<CapabilityId, CapabilityDef> => new Map(list.map((d) => [d.id, d]));

describe("Facets", () => {
  it("resolves a derived (combo) capability only when its prerequisites are held at minLevels", () => {
    const defs = defsOf([
      { id: "translate", held: "granted", facets: [], powerWeight: () => 0.2 },
      { id: "rearrange", held: "granted", facets: [], powerWeight: () => 0.3 },
      {
        id: "read-glyph",
        held: { derivedFrom: ["translate", "rearrange"], minLevels: { rearrange: 2 } },
        facets: [{ kind: "tag", tag: "glyph-passage" }],
        powerWeight: () => 0.6,
      },
    ]);
    const partial = buildHeld(defs, new Map([["translate", 1], ["rearrange", 1]]));
    expect(partial.hasCap("read-glyph")).toBe(false); // rearrange only level 1, needs 2
    const full = buildHeld(defs, new Map([["translate", 1], ["rearrange", 2]]));
    expect(full.hasCap("read-glyph")).toBe(true);
    expect(activeTags(defs, full).has("glyph-passage")).toBe(true);
  });

  it("aggregates Magnitude Facets per bucket, using the level", () => {
    const defs = defsOf([
      { id: "air-jump", held: "granted", powerWeight: () => 0.5, facets: [{ kind: "magnitude", bucket: "traversal.zUp", evaluate: (ctx) => ctx.level * 2 }] },
      { id: "grapple", held: "granted", powerWeight: () => 0.5, facets: [{ kind: "magnitude", bucket: "traversal.xyGap", evaluate: () => 3 }] },
    ]);
    const held = buildHeld(defs, new Map([["air-jump", 3], ["grapple", 1]]));
    const b = aggregateBuckets(defs, held);
    expect(b["traversal.zUp"]).toBe(6); // level 3 * 2
    expect(b["traversal.xyGap"]).toBe(3);
  });

  it("feeds a conservative resource capacity into the Magnitude context", () => {
    const defs = defsOf([
      {
        id: "space-jump",
        held: "granted",
        powerWeight: () => 0.7,
        facets: [
          { kind: "resource", poolId: "jump-charge", capacity: (ctx) => 3 + ctx.level, regenHint: "site" },
          { kind: "magnitude", bucket: "traversal.zUp", evaluate: (ctx) => ctx.resource?.capacity ?? 0 },
        ],
      },
    ]);
    const held = buildHeld(defs, new Map([["space-jump", 2]]));
    // capacity at level 2 = 5 → magnitude reads capacity 5
    expect(aggregateBuckets(defs, held)["traversal.zUp"]).toBe(5);
  });

  it("honors a TagFacet's optional evaluate", () => {
    const defs = defsOf([
      { id: "reveal", held: "granted", powerWeight: () => 0.6, facets: [{ kind: "tag", tag: "always-on" }, { kind: "tag", tag: "conditional", evaluate: () => false }] },
    ]);
    const tags = activeTags(defs, buildHeld(defs, new Map([["reveal", 1]])));
    expect(tags.has("always-on")).toBe(true);
    expect(tags.has("conditional")).toBe(false);
  });
});
