import { describe, it, expect } from "vitest";
import { Rng, type Vec3, type WorldBox } from "../math/index.js";
import { GenError } from "../errors.js";
import { sphere } from "../volume/sdf.js";
import { classifySurface } from "./surface-classify.js";
import { hasLineOfSight } from "./landmarks.js";
import { scatterSpace } from "./scatter.js";
import { DEFAULT_ANCHOR_KINDS, type ContentAnchorKind } from "./anchor-kinds.js";

const env = (r: number): WorldBox => ({ min: [-r, -r, -r], max: [r, r, r] });
const dist = (a: Vec3, b: Vec3): number => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

describe("surface classification", () => {
  it("maps open-facing normals to surface kinds", () => {
    expect(classifySurface([0, 0, 1])).toBe("floor");
    expect(classifySurface([0, 0, -1])).toBe("ceiling");
    expect(classifySurface([1, 0, 0])).toBe("wall");
    expect(classifySurface([0.3, 0, 0.4])).toBe("slope");
    expect(classifySurface([0.3, 0, -0.4])).toBe("overhang");
  });
});

describe("line of sight", () => {
  it("is clear across open space and blocked through solid", () => {
    const open = sphere([0, 0, 0], 20); // open inside
    expect(hasLineOfSight(open, [-10, 0, 0], [10, 0, 0])).toBe(true);
    // a solid ball in the middle blocks the ray (positive = solid)
    const wall = (p: Vec3): number => 3 - Math.hypot(p[0], p[1], p[2]); // positive near origin (solid)
    expect(hasLineOfSight(wall, [-10, 0, 0], [10, 0, 0])).toBe(false);
  });
});

describe("anchor scatter", () => {
  it("respects cross-kind separation and structural clearance", () => {
    const field = sphere([0, 0, 0], 8); // open inside; surface at |p|=8
    const socket: Vec3 = [8, 0, 0];
    const anchors = scatterSpace({
      field,
      envelope: env(9),
      res: 1.5,
      spaceId: "s",
      socketPositions: [socket],
      required: [],
      rng: new Rng("scatter"),
    });
    expect(anchors.length).toBeGreaterThan(0);
    for (let i = 0; i < anchors.length; i++) {
      const ai = anchors[i]!;
      const ki = DEFAULT_ANCHOR_KINDS[ai.kindId]!;
      // clearance from the socket
      expect(dist(ai.pos, socket)).toBeGreaterThanOrEqual(ki.clearanceFromStructural - 1e-6);
      for (let j = i + 1; j < anchors.length; j++) {
        const aj = anchors[j]!;
        const kj = DEFAULT_ANCHOR_KINDS[aj.kindId]!;
        const sep = Math.max(ki.minSeparation, kj.minSeparation);
        expect(dist(ai.pos, aj.pos)).toBeGreaterThanOrEqual(sep - 1e-6);
      }
    }
  });

  it("throws a loud GenError when a required anchor cannot be placed", () => {
    const field = sphere([0, 0, 0], 8);
    const kinds: Record<string, ContentAnchorKind> = {
      "gadget-pickup": { id: "gadget-pickup", allowedSurfaces: ["floor", "wall", "ceiling", "slope", "overhang"], minSeparation: 1000, clearanceFromStructural: 0, targetDensity: 0 },
    };
    let err: GenError | undefined;
    try {
      scatterSpace({
        field,
        envelope: env(9),
        res: 1.5,
        spaceId: "s",
        socketPositions: [],
        required: [{ kindId: "gadget-pickup" }, { kindId: "gadget-pickup" }], // 2nd can't fit (1000 sep)
        kinds,
        rng: new Rng("fail"),
      });
    } catch (e) {
      err = e as GenError;
    }
    expect(err).toBeInstanceOf(GenError);
    expect(err?.code).toBe("anchors.required-unplaceable");
  });
});
