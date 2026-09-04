/**
 * **The class tree** — `editor/classes.json`, generated from the manifest.
 *
 * ⚠ **Read, never asked for**, like the palette. `cargo xtask check` fails when it is stale, so the
 * editor's idea of the type tree cannot drift from the compiler's.
 *
 * ▶ **The palette says what can be *placed*; this says what can be *inspected*.** Two different
 * questions — the palette carries ops, so nothing in it names a base, an ancestry or a hook.
 */

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const editorRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** One field on a class. */
export interface Field {
  name: string;
  type: string;
  /** ⚠ Decides whether it **appears** at all. */
  exposed: boolean;
  /** ⚠ Decides whether it is **writable**. A value worth showing and one worth editing differ. */
  mutable: boolean;
  api: boolean;
  doc: string;
  /** The declared default, as prose or a literal. */
  default?: string;
}

/** One method. */
export interface Method {
  name: string;
  /** ⚠ A question the core asks — what generates the OVERRIDES list. */
  hook: boolean;
  api: boolean;
  returns: string;
  doc: string;
  /** What happens when the hook is left alone, in prose. */
  default?: string;
}

/** One class in the tree. */
export interface ClassDef {
  path: string;
  name: string;
  kind: "object" | "variant" | "struct" | "enum";
  extends?: string;
  /** Ancestors, **nearest first** — which is override order. */
  ancestry: string[];
  doc: string;
  fields: Field[];
  methods: Method[];
}

let cached: ClassDef[] | undefined;

/** Every class the manifest declares stable. */
export function classes(): ClassDef[] {
  if (cached) return cached;
  const at = path.join(editorRoot, "classes.json");
  let raw: string;
  try {
    raw = readFileSync(at, "utf8");
  } catch {
    throw new Error(
      `the class tree is missing at ${at} — run \`cargo xtask generate\` from the repository root`,
    );
  }
  cached = (JSON.parse(raw) as { classes: ClassDef[] }).classes;
  return cached;
}

/** One class by its path. */
export function classAt(classPath: string): ClassDef | undefined {
  return classes().find((c) => c.path === classPath);
}

/**
 * A class and its ancestors, nearest first.
 *
 * ⚠ **Nearest first is not cosmetic.** The declaration that applies is the closest one, so a panel
 * listing root-first would put `/Core/Object` at the top of every view and bury what matters.
 */
export function withAncestry(classPath: string): ClassDef[] {
  const self = classAt(classPath);
  if (!self) return [];
  return [self, ...self.ancestry.map(classAt).filter((c): c is ClassDef => c !== undefined)];
}
