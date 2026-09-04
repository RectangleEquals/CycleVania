/**
 * **The bindings client** — the editor's only way to reach the core.
 *
 * ⚠ **In-process, never a socket.** The addon is a native Node module, so the editor's server calls it
 * directly: a function call, not a request. There is no CycleVania service to run, start, or keep in
 * sync — a host embedding the same addon gets the same object.
 *
 * ▶ **The editor's own HTTP service is a different thing** and is allowed. The browser talks to *the
 * editor's* server (see `serve.ts`); the editor's server talks to the core in-process. What the design
 * rules out is a CycleVania-side service, which is what would put editor concerns back in Rust.
 *
 * # Why this file is thin on purpose
 *
 * ⚠ **Every function here should be a pass-through.** The moment one of them starts *computing*
 * something — deriving a value the core could have returned, patching around a missing call — that is
 * the first symptom of a hole in the binding surface, and the fix is a binding, not a helper. The Rust
 * side has a test that fails when the editor would need one (`editor_needs_no_service.rs`).
 */

import { createRequire } from "node:module";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..", "..");

/** What the addon exposes. Mirrors the napi surface in `cv-bindings/src/lib.rs`. */
export interface ProjectHandle {
  /** Every content file, relative to the content root, sorted. */
  content(): string[];
  /** Read one content file. */
  read(rel: string): string;
  /** Write one canonically, returning what was written. */
  write(rel: string, src: string): string;
  /** Check the project. Throws with the findings if it does not pass. */
  validate(): void;
  /** Every dial id it declares, sorted. */
  dials(): string[];
  /** One dial, tab-separated: id, owner, kind, default, effective, source. */
  dial(id: string): string;
  /** The recipe, as hex. Dials are part of it; the seed is not. */
  fingerprint(): string;
  /** Generate. Returns `fingerprint\tseed\tscopes`. */
  generate(seed: string): string;
}

interface Addon {
  version(): string;
  Project: {
    open(path: string): ProjectHandle;
    loadFromFile(path: string): ProjectHandle;
    create(at: string, from: string | null): ProjectHandle;
  };
}

/**
 * Where the built addon is.
 *
 * ⚠ **Resolved, not configured.** A path in a config file is a path that goes stale silently the first
 * time somebody moves a build directory; failing here with the command that fixes it is the shorter
 * road.
 */
function addonPath(): string {
  const candidates = [
    path.join(repoRoot, "build", "cyclevania.node"),
    path.join(repoRoot, "target", "debug", "cv_bindings.dll"),
    path.join(repoRoot, "target", "debug", "libcv_bindings.dylib"),
    path.join(repoRoot, "target", "debug", "libcv_bindings.so"),
  ];
  const found = candidates.find(existsSync);
  if (!found) {
    throw new Error(
      "the CycleVania addon is not built — run `npm run verify:addon` from the repository root",
    );
  }
  return found;
}

let cached: Addon | undefined;

/** Load the addon once. */
export function core(): Addon {
  cached ??= require(addonPath()) as Addon;
  return cached;
}

/** The core's version string. */
export function version(): string {
  return core().version();
}

/** Open a project from its `.cvproj`. */
export function open(projectPath: string): ProjectHandle {
  return core().Project.open(projectPath);
}

/** Open a cooked package — one file, the whole surface a shipped game needs. */
export function openCooked(packagePath: string): ProjectHandle {
  return core().Project.loadFromFile(packagePath);
}

/**
 * Create a project — from nothing, or by copying one that exists.
 *
 * ⚠ **The editor is the only thing that asks for this**, and it still does not implement it: a host
 * *loads* a project and never asks for one to be brought into existence, so creation is the editor's
 * to offer and the core's to perform. `from` names a preset or any project on disk, and its content is
 * **copied** — a new project sharing files with the preset it came from breaks when that preset
 * changes, and presets change, because they are also the acceptance tests.
 */
export function create(at: string, from?: string): ProjectHandle {
  return core().Project.create(at, from ?? null);
}

/** A dial row, as the Dials view renders it. */
export interface Dial {
  id: string;
  owner: string;
  kind: string;
  default: string;
  effective: string;
  source: string;
}

/**
 * Every dial, parsed from the tab-separated rows the binding returns.
 *
 * ⚠ **Splitting a string is not computing a value.** The binding chose the fields and their order; this
 * turns its line into an object and adds nothing. If a field the panel needs is missing, the fix is in
 * `cv-bindings`, not here — see the note at the top of this file.
 */
export function dials(project: ProjectHandle): Dial[] {
  return project.dials().map((id) => {
    const [rowId = id, owner = "", kind = "", def = "", effective = "", source = ""] = project
      .dial(id)
      .split("\t");
    return { id: rowId, owner, kind, default: def, effective, source };
  });
}

/** What a generate produced. */
export interface World {
  fingerprint: string;
  seed: string;
  scopes: number;
}

/** Generate a world. */
export function generate(project: ProjectHandle, seed: string): World {
  const [fingerprint = "", rolled = seed, scopes = "0"] = project.generate(seed).split("\t");
  return { fingerprint, seed: rolled, scopes: Number(scopes) };
}
