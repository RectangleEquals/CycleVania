/**
 * **M18 — the graph editor's rules.**
 *
 * ⚠ **The connection rule has no compiler behind it.** `cv-compile` deliberately does not type-check a
 * wire — *"Impossible is the editor's"* — so anything these rules permit reaches lowering unchecked.
 * That makes the *refusals* the load-bearing assertions here, not the acceptances.
 */

import { describe, expect, it } from "vitest";
import {
  CORE_NODES,
  ExpressionError,
  NotInPaletteError,
  checkExpression,
  dialNode,
  mayConnect,
  mergedPalette,
  nodeFor,
  widgetFor,
  type Pin,
} from "../server/graph.ts";
import { palette } from "../server/palette.ts";
import type { Dial } from "../server/bindings.ts";

const out = (type: string): Pin => ({ name: "out", type, dir: "out" });
const into = (type: string): Pin => ({ name: "in", type, dir: "in" });

const DIAL: Dial = {
  id: "/Content/Items/Hookshot.rope_length",
  owner: "Hookshot",
  kind: "Number",
  doc: "How far the rope reaches, in metres.",
  default: "30",
  effective: "30",
  source: "AUTHORED",
  overridden: false,
  outOfBounds: false,
  bounds: { min: 8, max: 200, softMin: null, hardMax: null, enumPath: null, enumValues: [] },
};

describe("P02 — a node exists because the palette offers it", () => {
  it("makes a node the palette has", () => {
    const any = palette()[0]!;
    expect(nodeFor(any.op).op).toBe(any.op);
  });

  it("refuses one it does not, because there is no text field to type a wrong name into", () => {
    // ⚠ The compiler's "no op named `array.is_emty`" finding exists for script-generated schematics.
    // Through the editor the mistake never becomes a document.
    expect(() => nodeFor("array.is_emty")).toThrow(NotInPaletteError);
  });

  it("names the ten core nodes and no While", () => {
    // ⚠ Every loop must be provably finite — a conditional loop is `For Range` with a stated maximum,
    // and the stated maximum is the point.
    expect(CORE_NODES).toContain("For Range");
    expect(CORE_NODES as readonly string[]).not.toContain("While");
  });
});

describe("P02a / P03a — the palette merges the project's own dials", () => {
  it("adds a dial read node addressed by owner and name", () => {
    const node = dialNode(DIAL);
    expect(node.op).toBe("/Content/Items/Hookshot.rope_length#dial");
  });

  it("gives it the dial's real type rather than a wrapper", () => {
    // ⚠ A wrapper would make every consumer unwrap — and the pin type is what rejects a bad
    // connection, so wrapping disables the check that makes the connection rule worth having.
    expect(dialNode(DIAL).outputs?.[0]?.type).toBe("float");
    expect(dialNode({ ...DIAL, kind: "Bool" }).outputs?.[0]?.type).toBe("bool");
  });

  it("replaces on rebuild rather than appending", () => {
    // ⚠ Appending would leave a deleted dial on the palette, offering a node whose read has nothing
    // behind it.
    const withOne = mergedPalette([DIAL]);
    const withNone = mergedPalette([]);
    expect(withOne.length).toBe(palette().length + 1);
    expect(withNone.length).toBe(palette().length);
    expect(withNone.some((n) => n.op.endsWith("#dial"))).toBe(false);
  });

  it("makes a dial node available to nodeFor only when it is in the merged set", () => {
    const merged = mergedPalette([DIAL]);
    expect(() => nodeFor(dialNode(DIAL).op)).toThrow(NotInPaletteError);
    expect(nodeFor(dialNode(DIAL).op, merged).op).toBe(dialNode(DIAL).op);
  });
});

describe("P04 — connection rules, where impossible means the wire does not draw", () => {
  it("runs out to in, never the reverse", () => {
    expect(mayConnect(into("float"), out("float")).ok).toBe(false);
    expect(mayConnect(out("float"), into("float")).ok).toBe(true);
  });

  it("keeps execution and data apart", () => {
    expect(mayConnect(out("exec"), into("float")).ok).toBe(false);
    expect(mayConnect(out("float"), into("exec")).ok).toBe(false);
    expect(mayConnect(out("exec"), into("exec")).ok).toBe(true);
  });

  it("refuses Kind to Ref, which is the example the design names", () => {
    // ⚠ A class is not an instance. The whole authoring model rests on the distinction.
    const verdict = mayConnect(out("Kind<Actor>"), into("Ref<Actor>"));
    expect(verdict.ok).toBe(false);
    if (!verdict.ok) expect(verdict.why).toMatch(/class|value/);
  });

  it("accepts a derived reference where a base is wanted, and not the reverse", () => {
    expect(mayConnect(out("Ref<Item>"), into("Ref<Actor>")).ok).toBe(true);
    const backwards = mayConnect(out("Ref<Actor>"), into("Ref<Item>"));
    expect(backwards.ok).toBe(false);
    if (!backwards.ok) expect(backwards.why).toContain("wrong way");
  });

  it("refuses an implicit numeric conversion and says to insert a node", () => {
    // ⚠ The compiler does not type-check, so a conversion permitted here reaches lowering unchecked —
    // a graph computing on a value the VM never converted is wrong in a way no finding reports.
    const verdict = mayConnect(out("int"), into("float"));
    expect(verdict.ok).toBe(false);
    if (!verdict.ok) expect(verdict.why).toContain("conversion node");
  });

  it("always gives a reason, so a refusal is never indistinguishable from a drag bug", () => {
    const refusals = [
      mayConnect(out("float"), into("exec")),
      mayConnect(out("Kind<Actor>"), into("Ref<Actor>")),
      mayConnect(out("String"), into("Vec3")),
    ];
    for (const r of refusals) {
      expect(r.ok).toBe(false);
      if (!r.ok) expect(r.why.length).toBeGreaterThan(10);
    }
  });
});

describe("P04a — the widget is a consequence of the type", () => {
  it("maps primitives and vectors", () => {
    expect(widgetFor("bool")).toEqual({ kind: "toggle" });
    expect(widgetFor("float")).toEqual({ kind: "number" });
    expect(widgetFor("String")).toEqual({ kind: "text" });
    expect(widgetFor("Vec3")).toEqual({ kind: "vector", components: 3 });
  });

  it("picks an Unlock asset-then-row", () => {
    // ⚠ A table is a file and a row is not addressable until the file is chosen — one combined picker
    // would have to offer every row of every table.
    expect(widgetFor("Asset<Unlock>")).toEqual({ kind: "asset-then-row", of: "Unlock" });
  });

  it("gives a Kind pin a class picker and a Ref pin a wire", () => {
    expect(widgetFor("Kind<Object>")).toEqual({ kind: "class-picker", of: "Object" });
    expect(widgetFor("Ref<Actor>")).toEqual({ kind: "wire-only" });
  });

  it("gives the same type the same widget every time", () => {
    // ⚠ If two pins of one type could differ, the widget carries information the type does not.
    expect(widgetFor("Vec3")).toEqual(widgetFor("Vec3"));
  });
});

describe("P05a — Expression", () => {
  it("allows scalar arithmetic", () => {
    expect(() => checkExpression("(base + 100 * tanks) / drain")).not.toThrow();
    expect(() => checkExpression("(v2 - v1) * q")).not.toThrow();
  });

  it("forbids member access, and says why the alternative is better", () => {
    // ⚠ Feeding `v1.x` into a pin named `x1` keeps the graph showing which components are involved
    // and lets the pin type reject bad input.
    expect(() => checkExpression("v1.x * v2.x")).toThrow(ExpressionError);
    try {
      checkExpression("v1.x * v2.x");
    } catch (e) {
      expect((e as Error).message).toContain("named pin");
    }
  });

  it("forbids a method call, because that is a node", () => {
    expect(() => checkExpression("origin.distance_to(target)")).toThrow(/method node/);
  });

  it("forbids a conditional, because that is a Branch", () => {
    expect(() => checkExpression("if x > 0 then a else b")).toThrow(/Branch/);
  });

  it("refuses an empty formula", () => {
    expect(() => checkExpression("   ")).toThrow(ExpressionError);
  });
});
