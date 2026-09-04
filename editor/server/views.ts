/**
 * **M17's panels — their data and their rules.**
 *
 * ⚠ **Layout is deliberately absent.** `10-editor.md` §10 wants real mockups for panel arrangement,
 * docking and navigation, and has not had them. Every decision in this file is layout-independent:
 * *which* facts a panel shows and *which* it refuses, not where they sit.
 *
 * ▶ **So the mockups, when they come, are laid over working behaviour** rather than blocking it — and
 * none of the rules below have to be rediscovered by whoever draws them.
 */

import { classAt, classes, withAncestry, type ClassDef, type Field } from "./classes.ts";
import { palette, type PaletteNode } from "./palette.ts";

// ---------------------------------------------------------------------------------------------
// P01 — the content browser
// ---------------------------------------------------------------------------------------------

/** How the browser was filtered. */
export interface BrowseFilter {
  /** Restrict to one kind of class. */
  kind?: ClassDef["kind"];
  /** Restrict to a subtree, e.g. `/Core/Component`. */
  subtree?: string;
  /** A substring of the name or doc. */
  text?: string;
}

/**
 * The content browser: the class tree, filtered.
 *
 * ⚠ **Filtered by kind and subtree, not by a hand-kept list.** A browser with its own idea of what
 * exists is a second opinion about the API surface, and it drifts the first time the manifest changes.
 */
export function browse(filter: BrowseFilter = {}): ClassDef[] {
  const needle = filter.text?.toLowerCase();
  return classes().filter((c) => {
    if (filter.kind && c.kind !== filter.kind) return false;
    if (filter.subtree && !c.path.startsWith(filter.subtree)) return false;
    if (needle && !`${c.name} ${c.doc}`.toLowerCase().includes(needle)) return false;
    return true;
  });
}

// ---------------------------------------------------------------------------------------------
// P02 — the inspector
// ---------------------------------------------------------------------------------------------

/** One row in the inspector. */
export interface InspectorRow {
  name: string;
  type: string;
  /** ⚠ False means **read-only**, and the row still appears. */
  writable: boolean;
  doc: string;
  /** What happens when nothing sets it — prose or a literal, never the word "inherited". */
  fallback?: string;
  /** Which class in the ancestry declared it. */
  from: string;
}

/**
 * The inspector for one class.
 *
 * ⚠ **`exposed` decides whether a field appears; `mutable` decides whether it is writable.** Two
 * facts, because a value worth showing and a value worth editing are different — collapsing them
 * either hides things a developer needs to read or offers edits that do nothing.
 *
 * ▶ **Inherited fields are included, nearest declaration winning.** A field a developer can set on an
 * `Actor` does not stop existing because `/Core/Object` declared it.
 */
export function inspect(classPath: string): InspectorRow[] {
  const chain = withAncestry(classPath);
  const seen = new Set<string>();
  const rows: InspectorRow[] = [];
  for (const c of chain) {
    for (const f of c.fields) {
      if (!f.exposed || seen.has(f.name)) continue;
      seen.add(f.name);
      rows.push({
        name: f.name,
        type: f.type,
        writable: f.mutable,
        doc: f.doc,
        fallback: f.default,
        from: c.path,
      });
    }
  }
  return rows;
}

// ---------------------------------------------------------------------------------------------
// P03 — the Viewport
// ---------------------------------------------------------------------------------------------

/** What the Viewport lists for a class. */
export interface ViewportEntry {
  name: string;
  path: string;
  /** ⚠ Answered from the declared field, **never from the class name**. */
  contributesCollision: boolean;
  /** Why the answer is what it is — the field that decided it. */
  because: string;
}

/**
 * Components and assets, and whether each contributes collision.
 *
 * ⚠ **Answered from the palette and the manifest, never from a class name.** *"It is called
 * MeshComponent so it must collide"* is the guess this panel exists to replace: a component
 * contributes collision when it **declares a collision field**, and a future component that does not
 * follow the naming convention still answers correctly.
 */
export function viewport(classPath: string): ViewportEntry[] {
  const chain = withAncestry(classPath);
  const componentish = classes().filter(
    (c) => c.path.includes("Component") || chain.some((k) => k.path === c.path),
  );
  return componentish.map((c) => {
    const collision: Field | undefined = c.fields.find((f) => /collision/i.test(f.name));
    return {
      name: c.name,
      path: c.path,
      contributesCollision: collision !== undefined,
      because: collision
        ? `declares \`${collision.name}\``
        : "declares no collision field, so it contributes none",
    };
  });
}

// ---------------------------------------------------------------------------------------------
// P04 — the OVERRIDES list
// ---------------------------------------------------------------------------------------------

/** One overridable hook. */
export interface Override {
  name: string;
  returns: string;
  doc: string;
  /** Which class in the ancestry asks it. */
  from: string;
  /**
   * What happens if it is left alone.
   *
   * ⚠ **Prose, never the word "inherited".** A developer needs to know *what happens*, not that
   * something happens — and the manifest carries the prose for exactly this.
   */
  otherwise: string;
}

/** A hook with no declared default still has to say something useful. */
const NO_DEFAULT_RECORDED = "nothing — the core has no fallback for this one";

/**
 * Every hook in the ancestry, pre-populated.
 *
 * ⚠ **Pre-populated, not discovered.** A developer should see the whole question set the core will ask
 * of this class before writing anything — an empty list that fills in as hooks are overridden hides
 * exactly the ones nobody thought about.
 */
export function overrides(classPath: string): Override[] {
  const chain = withAncestry(classPath);
  const seen = new Set<string>();
  const out: Override[] = [];
  for (const c of chain) {
    for (const m of c.methods) {
      if (!m.hook || seen.has(m.name)) continue;
      seen.add(m.name);
      out.push({
        name: m.name,
        returns: m.returns,
        doc: m.doc,
        from: c.path,
        otherwise: m.default ?? NO_DEFAULT_RECORDED,
      });
    }
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// P05 — the DIALS section
// ---------------------------------------------------------------------------------------------

/** The six bodies a dial row can have. */
export type DialKind = "number" | "range" | "adaptive" | "enum" | "curve" | "table";

/** ⚠ One row shape, six bodies. Switching kind **replaces** the body. */
export type DialBody =
  | { kind: "number"; default: number }
  | { kind: "range"; min: number; max: number; default: number }
  | { kind: "adaptive"; softMin: number; hardMax: number }
  | { kind: "enum"; path: string; values: string[]; default: string }
  | { kind: "curve"; table: string; row: string }
  | { kind: "table"; table: string; evaluate: string };

/** A dial as the DIALS section holds it. */
export interface DialRow {
  name: string;
  doc: string;
  body: DialBody;
}

/** Why a dial name was refused. */
export class DialNameError extends Error {}

/**
 * Check a dial name at creation.
 *
 * ⚠ **The DIALS section refuses where the naming lint only nudges**, and the difference is deliberate:
 * a lint fires on content that already exists and blocking it would make consistency a gate rather
 * than a nudge. Here nothing exists yet — and **the id this produces is what host code types forever**,
 * so the cheap moment to say no is before it has a caller.
 */
export function checkDialName(name: string): void {
  if (name.length === 0) throw new DialNameError("a dial needs a name");
  if (!/^[a-z][a-z0-9_]*$/.test(name)) {
    throw new DialNameError(
      `\`${name}\` is not a usable dial name — lower_snake_case, starting with a letter. ` +
        `This becomes \`<ClassName>.${name}\` in every host that reads it.`,
    );
  }
  if (name.endsWith("_")) throw new DialNameError(`\`${name}\` ends in an underscore`);
}

/** The default body for a kind — what a fresh row starts as. */
export function bodyFor(kind: DialKind): DialBody {
  switch (kind) {
    case "number":
      return { kind: "number", default: 0 };
    case "range":
      return { kind: "range", min: 0, max: 1, default: 0 };
    case "adaptive":
      return { kind: "adaptive", softMin: 0, hardMax: 1 };
    case "enum":
      return { kind: "enum", path: "", values: [], default: "" };
    case "curve":
      return { kind: "curve", table: "", row: "" };
    case "table":
      return { kind: "table", table: "", evaluate: "" };
  }
}

/**
 * Create a dial row.
 *
 * ⚠ **Only here.** The Dials *view* turns knobs and creates nothing — a dial is created where it
 * lives, on a Schematic or a Spine slot.
 */
export function createDial(name: string, kind: DialKind, doc = ""): DialRow {
  checkDialName(name);
  return { name, doc, body: bodyFor(kind) };
}

/**
 * Switch a dial's kind.
 *
 * ⚠ **The body is replaced, not migrated.** `Default=30` means nothing as a curve row, and carrying
 * a number across into a curve would produce a row that looks configured and is not.
 */
export function switchKind(row: DialRow, kind: DialKind): DialRow {
  return row.body.kind === kind ? row : { ...row, body: bodyFor(kind) };
}

/** The palette nodes a class contributes, for the browser's detail pane. */
export function nodesFor(classPath: string): PaletteNode[] {
  const name = classAt(classPath)?.name;
  if (!name) return [];
  return palette().filter((n) => n.op.startsWith(`${classPath}.`) || n.category === name);
}
