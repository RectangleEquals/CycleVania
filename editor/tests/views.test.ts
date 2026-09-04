/**
 * **M17's rules, tested against the real generated artifacts.**
 *
 * ⚠ **Against the real `classes.json`, not a fixture.** These panels exist to show what the manifest
 * actually declares; a fixture would let the rules pass while the data they read had drifted, which is
 * the exact failure the generated-artifact check exists to prevent.
 */

import { describe, expect, it } from "vitest";
import { classAt, classes, withAncestry } from "../server/classes.ts";
import {
  DialNameError,
  browse,
  checkDialName,
  createDial,
  inspect,
  overrides,
  switchKind,
  viewport,
} from "../server/views.ts";

describe("P01 — the content browser", () => {
  it("filters by kind", () => {
    const enums = browse({ kind: "enum" });
    expect(enums.length).toBeGreaterThan(0);
    expect(enums.every((c) => c.kind === "enum")).toBe(true);
  });

  it("filters by subtree", () => {
    const components = browse({ subtree: "/Core/Component" });
    expect(components.length).toBeGreaterThan(0);
    expect(components.every((c) => c.path.startsWith("/Core/Component"))).toBe(true);
  });

  it("filters by text across name and doc", () => {
    expect(browse({ text: "component" }).length).toBeGreaterThan(0);
    expect(browse({ text: "zzz-no-such-thing" })).toEqual([]);
  });

  it("shows everything when unfiltered", () => {
    expect(browse().length).toBe(classes().length);
  });
});

describe("P02 — the inspector", () => {
  it("shows exposed fields and hides the rest", () => {
    const rows = inspect("/Core/Actor");
    expect(rows.length).toBeGreaterThan(0);
    const declared = withAncestry("/Core/Actor").flatMap((c) => c.fields);
    const hidden = declared.filter((f) => !f.exposed).map((f) => f.name);
    for (const name of hidden) {
      expect(rows.some((r) => r.name === name), `${name} is not exposed`).toBe(false);
    }
  });

  it("marks a field writable only when it is mutable", () => {
    // ⚠ Two facts, not one: a value worth showing and a value worth editing are different.
    const rows = inspect("/Core/Actor");
    for (const row of rows) {
      const source = classAt(row.from);
      const field = source?.fields.find((f) => f.name === row.name);
      expect(row.writable).toBe(field?.mutable);
    }
  });

  it("includes inherited fields and says which class declared each", () => {
    // `/Core/Item` extends `/Core/Actor`, which declares exposed fields of its own.
    const rows = inspect("/Core/Item");
    const inherited = rows.filter((r) => r.from !== "/Core/Item");
    expect(inherited.length).toBeGreaterThan(0);
    expect(inherited.some((r) => r.from === "/Core/Actor")).toBe(true);
  });

  it("hides system-managed fields that a graph can still read", () => {
    // ⚠ **`api` and `exposed` are not the same flag, and this is the case that proves it.**
    // `Actor.components`, `parent` and `children` are `api = true` — the palette emits `get` nodes for
    // them, so a graph reads them — and they are **not** `exposed`, because nobody authors them: they
    // are what the system did. An inspector offering them would invite an edit that cannot happen.
    const rows = inspect("/Core/Actor").map((r) => r.name);
    for (const managed of ["components", "parent", "children"]) {
      expect(rows, managed).not.toContain(managed);
    }
    // And the ones a developer really does set are there.
    expect(rows).toContain("transform");
  });

  it("renders a default as prose, never the word inherited", () => {
    const withFallback = classes()
      .flatMap((c) => inspect(c.path))
      .filter((r) => r.fallback !== undefined);
    expect(withFallback.length).toBeGreaterThan(0);
    for (const row of withFallback) {
      expect(row.fallback?.toLowerCase()).not.toContain("inherited");
    }
  });
});

describe("P03 — the Viewport", () => {
  it("answers collision from a declared field, not from a class name", () => {
    const entries = viewport("/Core/Actor");
    const mesh = entries.find((e) => e.name === "MeshComponent");
    expect(mesh?.contributesCollision).toBe(true);
    expect(mesh?.because).toContain("collision");

    // ⚠ The point of the rule: something *named* like a component that declares no collision field
    // still answers no. A name is not evidence.
    const plain = entries.find((e) => e.name === "Component");
    expect(plain?.contributesCollision).toBe(false);
    expect(plain?.because).toContain("no collision field");
  });
});

describe("P04 — the OVERRIDES list", () => {
  it("is pre-populated from every hook in the ancestry", () => {
    const list = overrides("/Core/Actor");
    expect(list.length).toBeGreaterThan(0);
    // Actor asks its own, and inherits Object's.
    expect(list.some((o) => o.from === "/Core/Actor")).toBe(true);
    expect(list.some((o) => o.from === "/Core/Object")).toBe(true);
  });

  it("lists only hooks, never ordinary methods", () => {
    for (const o of overrides("/Core/Actor")) {
      const source = classAt(o.from);
      expect(source?.methods.find((m) => m.name === o.name)?.hook).toBe(true);
    }
  });

  it("says what happens instead, in prose, for a hook left alone", () => {
    // ⚠ A developer needs to know *what happens*, not that something happens.
    const list = overrides("/Core/Actor");
    const withProse = list.filter((o) => o.otherwise !== "" && !o.otherwise.startsWith("nothing —"));
    expect(withProse.length).toBeGreaterThan(0);
    for (const o of list) {
      expect(o.otherwise.toLowerCase()).not.toBe("inherited");
      expect(o.otherwise.length).toBeGreaterThan(0);
    }
  });

  it("declares each hook once, nearest declaration winning", () => {
    const list = overrides("/Core/Actor");
    expect(new Set(list.map((o) => o.name)).size).toBe(list.length);
  });
});

describe("P05 — the DIALS section", () => {
  it("refuses a name that would become a bad host-facing id", () => {
    // ⚠ Refused here where the lint only nudges: nothing exists yet, and the id this produces is what
    // host code types forever.
    for (const bad of ["", "Speed", "3speed", "speed-2", "speed ", "speed_"]) {
      expect(() => checkDialName(bad), bad).toThrow(DialNameError);
    }
  });

  it("accepts lower_snake_case", () => {
    for (const good of ["speed", "rope_length", "max_charge_2"]) {
      expect(() => checkDialName(good), good).not.toThrow();
    }
  });

  it("creates one row shape with six possible bodies", () => {
    const kinds = ["number", "range", "adaptive", "enum", "curve", "table"] as const;
    for (const kind of kinds) {
      const row = createDial("speed", kind);
      expect(row.name).toBe("speed");
      expect(row.body.kind).toBe(kind);
    }
  });

  it("replaces the body when the kind changes rather than migrating it", () => {
    // ⚠ `Default=30` means nothing as a curve row, and carrying it across would produce a row that
    // looks configured and is not.
    const number = createDial("speed", "number");
    const curve = switchKind(number, "curve");
    expect(curve.body.kind).toBe("curve");
    expect(curve.body).not.toHaveProperty("default");
    expect(curve.name).toBe("speed");
  });

  it("leaves a row alone when the kind does not change", () => {
    const row = createDial("speed", "range");
    expect(switchKind(row, "range")).toBe(row);
  });
});
