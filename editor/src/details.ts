/**
 * **The Details panel** — Godot's inspector, with Unreal's grouping.
 *
 * ⚠ **§9a named Godot as the exception and specified nothing**, so a first draft of this panel was a
 * stack of cards. ▶ **What Godot's inspector actually does maps onto data M17 already computes**, which
 * is why it is the right reference and not merely a nice one.
 *
 * # Both engines band, on different things — `10-editor.md` §9c
 *
 * - ▶ **Godot bands by *class in the ancestry***, so a property says which class declared it
 * - ▶ **Unreal groups by *category*** — `Transform`, `Collision` — each ending in a folded `Advanced`
 * - ⚠ **CycleVania has the data for both**: class bands outer, category groups inner, which answers
 *   *"this comes from `Actor`, and within `Actor` it is about collision"* — a question neither engine
 *   alone answers.
 *
 * # The rules that are not style
 *
 * - ⚠ **`exposed` decides whether a row appears; `mutable` decides whether it is editable.** A
 *   non-`mutable` row is **greyed and readable**, never hidden — *"you may not change this"* and
 *   *"this does not exist"* are different answers.
 * - ⚠ **The widget is a consequence of the type**, never a per-field choice.
 * - ▶ **A revert arrow appears on a property that differs from its default, and only then.**
 */

import { icon } from "./icons.ts";
import { pinColour } from "./pins.ts";

export interface FieldDef {
  name: string;
  type: string;
  exposed: boolean;
  mutable: boolean;
  api: boolean;
  doc: string;
  default?: string;
  /** Unreal's grouping. ⚠ Absent means the class's own band, ungrouped. */
  category?: string;
  /** ⚠ Folded by default — §9d applied to a panel. */
  advanced?: boolean;
}

export interface ClassDef {
  path: string;
  name: string;
  kind: string;
  ancestry: string[];
  doc: string;
  fields: FieldDef[];
  values?: { name: string; doc: string }[];
}

/** The subject the panel is describing. */
export interface Subject {
  label: string;
  icon: string;
  classPath: string;
  /** Current values, by field name. Absent means "still the default". */
  values: Record<string, string>;
  /** ▶ **Provenance is a link** — which asset put this here. */
  from?: { label: string; path: string };
}

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

// ---------------------------------------------------------------------------------------------
// Which rows appear, and under which band
// ---------------------------------------------------------------------------------------------

/** A class band: the class that declares these fields, and the fields it declares. */
export interface Band {
  path: string;
  name: string;
  fields: FieldDef[];
}

/**
 * Build the bands for a subject.
 *
 * ⚠ **Nearest class first** — Godot orders from the concrete class outward, because that is where a
 * developer looks first. ▶ **A band with no exposed field is not drawn**: §9d's rule, and it is what
 * stops `/Core/Object` contributing an empty header to every object in the project.
 */
export function bands(classes: ClassDef[], classPath: string): Band[] {
  const byPath = new Map(classes.map((c) => [c.path, c]));
  const self = byPath.get(classPath);
  if (!self) return [];
  const chain = [self.path, ...self.ancestry];
  return chain
    .map((p) => byPath.get(p))
    .filter((c): c is ClassDef => !!c)
    .map((c) => ({ path: c.path, name: c.name, fields: c.fields.filter((f) => f.exposed) }))
    .filter((b) => b.fields.length > 0);
}

/**
 * The filter.
 *
 * ⚠ **It reaches inside collapsed groups and expands what it finds.** A search that only matches what
 * is already visible fails exactly when it is needed. ▶ It matches the name *and the doc*, because a
 * developer often remembers what a property does rather than what it is called.
 */
export function filterBands(bs: Band[], q: string): Band[] {
  const needle = q.trim().toLowerCase();
  if (!needle) return bs;
  return bs
    .map((b) => ({
      ...b,
      fields: b.fields.filter(
        (f) =>
          f.name.toLowerCase().includes(needle) ||
          f.type.toLowerCase().includes(needle) ||
          f.doc.toLowerCase().includes(needle),
      ),
    }))
    .filter((b) => b.fields.length > 0);
}

/** Whether a value differs from the declared default. ⚠ This is what the revert arrow keys on. */
export function isOverridden(f: FieldDef, values: Record<string, string>): boolean {
  const v = values[f.name];
  if (v === undefined) return false;
  return v !== (f.default ?? "");
}

// ---------------------------------------------------------------------------------------------
// The widget is a consequence of the type
// ---------------------------------------------------------------------------------------------

export type Widget =
  | "check"
  | "drag"
  | "text"
  | "dropdown"
  | "asset"
  | "curve"
  | "list"
  | "map";

/**
 * Which editor a type gets.
 *
 * ⚠ **Never a per-field choice** — the same rule `10-editor.md` states for a graph's `Unlock` pin,
 * applied to the panel.
 *
 * ⚠ **An enum is a dropdown, always.** A draft copied Unreal's `Static / Stationary / Movable` as a
 * segmented group whenever there were three options or fewer — ▶ **but a rule that changes shape with
 * the option count does not scale.** `/Core/Face` already has six, and nothing stops a project's enum
 * having twenty; the panel would grow a wall of buttons exactly where it should stay quiet. **That
 * inverts §9d**: depth belongs behind a drill-down, not spread across the surface.
 */
export function widgetFor(type: string, variants = 0): Widget {
  if (type === "bool") return "check";
  if (type === "int" || type === "float") return "drag";
  if (type.startsWith("Array<")) return "list";
  if (type.startsWith("Map<")) return "map";
  if (type.startsWith("Kind<") || type.startsWith("Ref<")) return "asset";
  if (type === "CurveRef" || type.startsWith("Curve")) return "curve";
  if (variants > 0) return "dropdown";
  return "text";
}

/** ⚠ **The picker's filter is `T`** — the visible form of the graph's connection rule. */
export function assetFilter(type: string): string {
  const m = /^(?:Kind|Ref)<(.+)>$/.exec(type);
  return m ? m[1]! : type;
}

function widget(f: FieldDef, value: string, variants: { name: string }[], editable: boolean): string {
  const w = widgetFor(f.type, variants.length);
  const dis = editable ? "" : " disabled";
  switch (w) {
    case "check":
      return (
        `<label class="cv-w"><input type="checkbox" ${value === "true" ? "checked" : ""}${dis}/>` +
        `<span>${value === "true" ? "On" : "Off"}</span></label>`
      );
    case "drag":
      // ▶ **A drag-field**: a text box you can also scrub. The class is what makes it one.
      return `<input class="cv-w cv-drag" value="${esc(value)}"${dis}/>`;
    case "dropdown":
      return (
        `<select class="cv-w"${dis}>` +
        variants
          .map((v) => `<option ${v.name === value ? "selected" : ""}>${esc(v.name)}</option>`)
          .join("") +
        `</select>`
      );
    case "asset":
      // ⚠ The picker names what it will accept, so the constraint is visible before the click.
      return (
        `<button class="cv-w cv-pick"${dis}>` +
        `<span class="cv-pdot" style="background:${pinColour(f.type)}"></span>` +
        `<span>${esc(value || "None")}</span>` +
        `<span class="cv-pfilter">${esc(assetFilter(f.type))}</span></button>`
      );
    case "curve":
      return `<button class="cv-w cv-pick"${dis}>${icon("curve", 13)}<span>${esc(value || "None")}</span></button>`;
    case "list":
      return `<button class="cv-w cv-pick"${dis}><span>${esc(value || "0 elements")}</span><span class="cv-pfilter">${esc(assetFilter(f.type.slice(6, -1)))}</span></button>`;
    case "map":
      return `<button class="cv-w cv-pick"${dis}><span>${esc(value || "0 entries")}</span></button>`;
    default:
      return `<input class="cv-w" value="${esc(value)}"${dis}/>`;
  }
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

export interface DetailsState {
  search: string;
  /** Class bands the developer folded. */
  folded: Record<string, boolean>;
  /** ⚠ `Advanced` starts folded — §9d. */
  advancedOpen: Record<string, boolean>;
}

/** Draw the panel. */
export function drawDetails(
  classes: ClassDef[],
  subject: Subject | null,
  state: DetailsState,
): string {
  if (!subject) {
    return `<div class="cv-empty">Select something to see its details.</div>`;
  }
  const byPath = new Map(classes.map((c) => [c.path, c]));
  const variantsOf = (type: string) => byPath.get(`/Core/${type}`)?.values ?? [];

  const all = bands(classes, subject.classPath);
  const shown = filterBands(all, state.search);
  const expanding = state.search.trim().length > 0;

  const row = (f: FieldDef) => {
    const value = subject.values[f.name] ?? f.default ?? "";
    const over = isOverridden(f, subject.values);
    return (
      `<div class="cv-drow${f.mutable ? "" : " is-locked"}" title="${esc(f.doc)}">` +
      `<span class="cv-dname">${esc(f.name)}</span>` +
      `<span class="cv-dval">${widget(f, value, variantsOf(f.type), f.mutable)}</span>` +
      // ▶ **Only when it differs from the default.** CycleVania already calls this `overridden`.
      (over ? `<button class="cv-revert" title="Reset to ${esc(f.default ?? "default")}">↩</button>` : `<span class="cv-revert-gap"></span>`) +
      `</div>`
    );
  };

  const band = (b: Band) => {
    const folded = !expanding && state.folded[b.path];
    const plain = b.fields.filter((f) => !f.advanced);
    const adv = b.fields.filter((f) => f.advanced);
    const advOpen = expanding || state.advancedOpen[b.path];
    return (
      `<div class="cv-band">` +
      `<button class="cv-bandhead" data-band="${esc(b.path)}">` +
      `<span class="cv-caret">${folded ? "▸" : "▾"}</span>` +
      `<span>${esc(b.name)}</span>` +
      `<span class="cv-bandpath">${esc(b.path)}</span></button>` +
      (folded
        ? ""
        : plain.map(row).join("") +
          (adv.length
            ? `<button class="cv-adv" data-adv="${esc(b.path)}">` +
              `<span class="cv-caret">${advOpen ? "▾" : "▸"}</span>Advanced ` +
              `<span class="cv-ocount">${adv.length}</span></button>` +
              (advOpen ? adv.map(row).join("") : "")
            : "")) +
      `</div>`
    );
  };

  return (
    // ⚠ **A panel that does not say what it describes cannot be trusted.**
    `<div class="cv-subject">${icon(subject.icon, 14)}<span>${esc(subject.label)}</span></div>` +
    `<div class="cv-dclass">${esc(subject.classPath)}` +
    // ▶ *"Which schematic put this here"* is the first question anybody asks of a generated level.
    (subject.from
      ? `<button class="cv-prov" data-open="${esc(subject.from.path)}">${esc(subject.from.label)}</button>`
      : "") +
    `</div>` +
    `<input class="cv-search cv-dsearch" placeholder="Filter properties" value="${esc(state.search)}"/>` +
    (shown.length
      ? shown.map(band).join("")
      : `<div class="cv-empty">No property matches “${esc(state.search)}”.</div>`)
  );
}

export function detailsStyles(): string {
  const V = (n: string) => `var(--cv-${n})`;
  return `
.cv-dclass { display: flex; align-items: center; gap: 8px; padding: 0 6px 7px; color: ${V("muted")};
  font-size: 10.5px; font-family: ui-monospace, monospace; }
.cv-prov { margin-left: auto; background: none; border: 0; color: ${V("accent")}; font: inherit;
  cursor: pointer; padding: 0; text-decoration: underline; }
.cv-dsearch { margin: 0 6px 8px; width: calc(100% - 12px); }

/* WARN **Class bands outer, category groups inner** — SS9c. Godot bands by class; Unreal by category. */
.cv-band { margin-bottom: 2px; }
.cv-bandhead { display: flex; align-items: center; gap: 6px; width: 100%; background: ${V("raised")};
  border: 0; border-top: 1px solid ${V("line")}; color: ${V("text")}; font: inherit; cursor: pointer;
  padding: 5px 8px; text-align: left; font-size: 11.5px; font-weight: 600; }
.cv-bandhead:hover { background: ${V("selected")}; }
.cv-bandhead:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: -2px; }
.cv-bandpath { margin-left: auto; color: ${V("muted")}; font-weight: 400; font-size: 10px;
  font-family: ui-monospace, monospace; }

.cv-drow { display: grid; grid-template-columns: 40% 1fr auto; gap: 6px; align-items: center;
  padding: 3px 8px; }
.cv-drow:hover { background: ${V("raised")}; }
.cv-dname { font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
/* WARN **Greyed and readable, never hidden** — *"you may not change this"* is not *"this does not exist"*. */
.cv-drow.is-locked .cv-dname, .cv-drow.is-locked .cv-w { opacity: .55; }
.cv-dval { min-width: 0; }
.cv-w { width: 100%; background: ${V("bg")}; color: ${V("text")}; border: 1px solid ${V("line")};
  border-radius: ${V("radius")}; padding: 2px 6px; font: inherit; font-size: 11.5px; }
.cv-w:focus { outline: none; border-color: ${V("accent")}; }
label.cv-w { display: flex; align-items: center; gap: 6px; background: none; border: 0; padding: 2px 0; }
.cv-drag { cursor: ew-resize; font-family: ui-monospace, monospace; }
.cv-pick { display: flex; align-items: center; gap: 6px; text-align: left; cursor: pointer; }
.cv-pdot { width: 8px; height: 8px; border-radius: 50%; flex: 0 0 auto; }
.cv-pfilter { margin-left: auto; color: ${V("muted")}; font-size: 10px;
  font-family: ui-monospace, monospace; }
.cv-revert { background: none; border: 0; color: ${V("accent")}; cursor: pointer; font: inherit;
  padding: 0 2px; line-height: 1; }
.cv-revert:hover { color: ${V("text")}; }
.cv-revert-gap { display: inline-block; width: 13px; }
.cv-adv { display: flex; align-items: center; gap: 5px; width: 100%; background: none; border: 0;
  color: ${V("muted")}; font: inherit; font-size: 11px; cursor: pointer; padding: 4px 8px;
  text-align: left; }
.cv-adv:hover { color: ${V("text")}; }
`;
}
