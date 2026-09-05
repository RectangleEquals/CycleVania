/**
 * **The Details panel.**
 *
 * ⚠ **The assertions are about the two facts M17 proved are different** — `exposed` decides whether a
 * row appears, `mutable` whether it is editable — and about the bands, which are that rule *drawn*: a
 * property's class band is the answer to *"where did this come from"*.
 */

import { describe, expect, it } from "vitest";
import {
  assetFilter,
  bands,
  detailsStyles,
  drawDetails,
  filterBands,
  isOverridden,
  widgetFor,
  type ClassDef,
  type DetailsState,
  type FieldDef,
  type Subject,
} from "../src/details.ts";

const f = (over: Partial<FieldDef> & { name: string; type: string }): FieldDef => ({
  exposed: true,
  mutable: true,
  api: true,
  doc: "",
  ...over,
});

const CLASSES: ClassDef[] = [
  {
    path: "/Core/Object",
    name: "Object",
    kind: "object",
    ancestry: [],
    doc: "",
    // ⚠ All system-managed — this band must not be drawn at all.
    fields: [f({ name: "id", type: "ObjectId", exposed: false, mutable: false })],
  },
  {
    path: "/Core/Actor",
    name: "Actor",
    kind: "object",
    ancestry: ["/Core/Object"],
    doc: "",
    fields: [
      f({ name: "components", type: "Array<Ref<Object>>", exposed: true, mutable: false }),
      f({ name: "mount_face", type: "Face" }),
      f({ name: "collision_mode", type: "CollisionMode" }),
      f({ name: "hidden", type: "bool", default: "false" }),
      f({ name: "tint", type: "Kind<Surface>" }),
      f({ name: "net_priority", type: "float", default: "1.0", advanced: true }),
    ],
  },
  {
    path: "/Core/Item",
    name: "Item",
    kind: "object",
    ancestry: ["/Core/Actor", "/Core/Object"],
    doc: "",
    fields: [f({ name: "quantity", type: "int", default: "1", doc: "how many are granted" })],
  },
  {
    path: "/Core/Face",
    name: "Face",
    kind: "enum",
    ancestry: [],
    doc: "",
    fields: [],
    values: [{ name: "POS_X", doc: "" }, { name: "NEG_X", doc: "" }, { name: "POS_Y", doc: "" },
             { name: "NEG_Y", doc: "" }, { name: "POS_Z", doc: "" }, { name: "NEG_Z", doc: "" }],
  },
  {
    path: "/Core/CollisionMode",
    name: "CollisionMode",
    kind: "enum",
    ancestry: [],
    doc: "",
    fields: [],
    values: [{ name: "NONE", doc: "" }, { name: "QUERY", doc: "" }, { name: "BLOCK", doc: "" }],
  },
];

const subject: Subject = {
  label: "Hookshot",
  icon: "schematic",
  classPath: "/Core/Item",
  values: { quantity: "3" },
  from: { label: "Hookshot.cvs", path: "schematics/Hookshot.cvs" },
};

const state = (over: Partial<DetailsState> = {}): DetailsState => ({
  search: "",
  folded: {},
  advancedOpen: {},
  ...over,
});

describe("class bands — M17 P02's rule, drawn", () => {
  it("orders from the concrete class outward", () => {
    // ▶ Godot's order, and the reason is where a developer looks first.
    expect(bands(CLASSES, "/Core/Item").map((b) => b.name)).toEqual(["Item", "Actor"]);
  });

  it("does not draw a band whose fields are all system-managed", () => {
    // ⚠ **`/Core/Object` contributes nothing exposed**, so an empty header would ride on every object
    // in the project — §9d's rule at panel scale.
    expect(bands(CLASSES, "/Core/Item").some((b) => b.path === "/Core/Object")).toBe(false);
  });

  it("names the class each property came from", () => {
    const html = drawDetails(CLASSES, subject, state());
    expect(html).toContain("/Core/Actor");
    expect(html).toContain("/Core/Item");
  });

  it("says what it is describing", () => {
    // ⚠ **A panel that does not say what it describes cannot be trusted.**
    expect(drawDetails(CLASSES, subject, state())).toContain("Hookshot");
  });

  it("says so plainly with nothing selected", () => {
    expect(drawDetails(CLASSES, null, state())).toContain("Select something");
  });
});

describe("exposed decides appearance; mutable decides editability", () => {
  it("hides what is not exposed", () => {
    // ⚠ M17 proved these are different questions: **41 fields are `api` and not `exposed`.**
    expect(drawDetails(CLASSES, subject, state())).not.toContain(">id<");
  });

  it("shows a non-mutable field greyed and readable, never hidden", () => {
    // ⚠ *"You may not change this"* and *"this does not exist"* are different answers.
    const html = drawDetails(CLASSES, subject, state());
    expect(html).toContain("components");
    expect(html).toMatch(/cv-drow is-locked[^>]*>[^<]*<span class="cv-dname">components/);
    expect(html).toContain("disabled");
  });
});

describe("the widget is a consequence of the type", () => {
  it("picks by type, never per field", () => {
    expect(widgetFor("bool")).toBe("check");
    expect(widgetFor("int")).toBe("drag");
    expect(widgetFor("float")).toBe("drag");
    expect(widgetFor("Array<Tag>")).toBe("list");
    expect(widgetFor("Map<String, Kind<Surface>>")).toBe("map");
    expect(widgetFor("Kind<Surface>")).toBe("asset");
    expect(widgetFor("Ref<Object>")).toBe("asset");
  });

  it("gives every enum a dropdown, whatever the option count", () => {
    // ⚠ **A rule that changes shape with the option count does not scale.** A draft used a
    // segmented group at three or fewer, copying Unreal's `Static / Stationary / Movable`; ▶
    // `/Core/Face` already has six and a project's enum could have twenty, so the panel would grow a
    // wall of buttons exactly where it should stay quiet. **That inverts §9d.**
    for (const n of [2, 3, 4, 6, 20]) expect(widgetFor("SomeEnum", n), String(n)).toBe("dropdown");
  });

  it("never renders an enum as a row of buttons", () => {
    expect(drawDetails(CLASSES, subject, state())).not.toContain("cv-seg");
  });

  it("renders an enum from the manifest's variants, not from a guess", () => {
    // ⚠ **The artifact carried no variants until this milestone** — 15 enum classes reached
    // `classes.json` empty, and a dropdown cannot be drawn from a type name.
    const html = drawDetails(CLASSES, subject, state());
    expect(html).toContain("<select");
    expect(html).toContain("POS_Y");
    expect(html).toContain("BLOCK");
  });

  it("names what an asset picker will accept", () => {
    // ⚠ **The picker's filter is `T`** — the visible form of the graph's connection rule.
    expect(assetFilter("Kind<Surface>")).toBe("Surface");
    expect(assetFilter("Ref<Object>")).toBe("Object");
    expect(drawDetails(CLASSES, subject, state())).toContain("Surface");
  });
});

describe("the revert arrow, and what it keys on", () => {
  it("appears only where the value differs from the default", () => {
    // ▶ CycleVania already has this concept and calls it `overridden`.
    expect(isOverridden(CLASSES[2]!.fields[0]!, { quantity: "3" })).toBe(true);
    expect(isOverridden(CLASSES[2]!.fields[0]!, { quantity: "1" })).toBe(false);
    expect(isOverridden(CLASSES[2]!.fields[0]!, {})).toBe(false);
  });

  it("draws one arrow for the one changed property", () => {
    const html = drawDetails(CLASSES, subject, state());
    expect(html.split("cv-revert\"").length - 1).toBe(1);
  });
});

describe("the filter reaches inside what is folded", () => {
  it("matches name, type and doc", () => {
    // ▶ A developer often remembers what a property *does* rather than what it is called.
    expect(filterBands(bands(CLASSES, "/Core/Item"), "granted")[0]!.fields[0]!.name).toBe("quantity");
    expect(filterBands(bands(CLASSES, "/Core/Item"), "bool")[0]!.fields[0]!.name).toBe("hidden");
  });

  it("drops a band with no match rather than leaving an empty header", () => {
    expect(filterBands(bands(CLASSES, "/Core/Item"), "quantity").map((b) => b.name)).toEqual(["Item"]);
  });

  it("expands a folded band when the search finds something inside it", () => {
    // ⚠ **A search that only matches what is already visible fails exactly when it is needed.**
    const folded = state({ folded: { "/Core/Actor": true } });
    expect(drawDetails(CLASSES, subject, folded)).not.toContain("mount_face");
    expect(drawDetails(CLASSES, subject, { ...folded, search: "mount" })).toContain("mount_face");
  });

  it("says so when nothing matches", () => {
    expect(drawDetails(CLASSES, subject, state({ search: "zzz" }))).toContain("No property matches");
  });
});

describe("Advanced folds", () => {
  it("is closed by default", () => {
    // ⚠ **§9d's rule, not a style choice.** The fold says *"there is more, and you do not need it now"*.
    const html = drawDetails(CLASSES, subject, state());
    expect(html).toContain("Advanced");
    expect(html).not.toContain("net_priority");
  });

  it("opens when asked, and when the search reaches into it", () => {
    expect(drawDetails(CLASSES, subject, state({ advancedOpen: { "/Core/Actor": true } })))
      .toContain("net_priority");
    expect(drawDetails(CLASSES, subject, state({ search: "net_" }))).toContain("net_priority");
  });
});

describe("provenance", () => {
  it("says which asset put this here, as a link", () => {
    // ⚠ **The first question anybody asks of a generated level.**
    expect(drawDetails(CLASSES, subject, state())).toContain("schematics/Hookshot.cvs");
  });
});

describe("the panel keeps the theme's promise", () => {
  it("takes every colour from the four parameters", () => {
    const bare = detailsStyles().replace(/var\(--cv-[a-z]+\)/g, "");
    expect(bare).not.toMatch(/#[0-9a-fA-F]{3,6}/);
  });
});
