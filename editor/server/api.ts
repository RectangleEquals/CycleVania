/**
 * **The editor's own API** — what the browser half calls.
 *
 * ⚠ **Every route here is a pass-through to the bindings.** That is the property worth protecting: if a
 * route ever needs to *compute* an answer, the core could not give it one, and the fix is a binding
 * rather than a smarter route. `cv-bindings/tests/editor_needs_no_service.rs` fails when that stops
 * being true.
 *
 * ▶ **This is the editor's service, not CycleVania's.** It exists because a browser cannot load a
 * native Node addon, and because a tablet on the LAN needs somewhere to connect. A *host project* needs
 * none of it — it embeds the addon and calls the same functions directly.
 */

import * as core from "./bindings.ts";
import { byCategory, palette } from "./palette.ts";
import type { ProjectHandle } from "./bindings.ts";

/** What the editor is currently holding open. */
export interface Session {
  path: string;
  project: ProjectHandle;
}

/** A route's answer: a status and a JSON body. */
export interface Reply {
  status: number;
  body: unknown;
}

const ok = (body: unknown): Reply => ({ status: 200, body });
const fail = (status: number, error: string): Reply => ({ status, body: { error } });

/**
 * Turn a thrown binding error into a reply.
 *
 * ⚠ **The core's message is passed through verbatim.** It already says which file, which finding and
 * why; rewriting it here would produce a second vocabulary for the same faults, and the developer would
 * have to learn both.
 */
function attempt(run: () => unknown): Reply {
  try {
    return ok(run());
  } catch (e) {
    return fail(400, e instanceof Error ? e.message : String(e));
  }
}

/**
 * The editor's routes.
 *
 * ⚠ **State is one open project, deliberately.** The design has no notion of a remote project or of
 * several at once: the editor serves one project on one machine's disk, and a session list would be the
 * first half of an accounts system nobody asked for.
 */
export class Api {
  #session: Session | undefined;

  /** What is open, if anything. */
  get session(): Session | undefined {
    return this.#session;
  }

  /** The core's version — the cheapest proof the addon actually loaded. */
  version(): Reply {
    return attempt(() => ({ version: core.version() }));
  }

  /** Open a project. */
  open(path: string): Reply {
    return attempt(() => {
      const project = core.open(path);
      this.#session = { path, project };
      return { path, content: project.content() };
    });
  }

  /**
   * Create a project, optionally from a preset or another project.
   *
   * ⚠ **The editor-only route.** A host loads a project; it never asks for one to be brought into
   * existence. Creation still happens in the core, because it writes the format.
   */
  create(at: string, from?: string): Reply {
    return attempt(() => {
      const project = core.create(at, from);
      this.#session = { path: at, project };
      return { path: at, content: project.content() };
    });
  }

  /** Every content file in the open project. */
  content(): Reply {
    const s = this.#session;
    if (!s) return fail(409, "no project is open");
    return attempt(() => ({ content: s.project.content() }));
  }

  /** Read one content file. */
  read(rel: string): Reply {
    const s = this.#session;
    if (!s) return fail(409, "no project is open");
    return attempt(() => ({ rel, text: s.project.read(rel) }));
  }

  /**
   * Write one content file.
   *
   * ⚠ **Returns what was actually written**, which is the canonical form and may differ from what was
   * sent. An editor that assumed its own text landed byte-for-byte would show a buffer that disagrees
   * with the file the moment the writer normalises anything — and the writer normalises key order.
   */
  write(rel: string, text: string): Reply {
    const s = this.#session;
    if (!s) return fail(409, "no project is open");
    return attempt(() => ({ rel, written: s.project.write(rel, text) }));
  }

  /** Validate the open project. */
  validate(): Reply {
    const s = this.#session;
    if (!s) return fail(409, "no project is open");
    return attempt(() => {
      s.project.validate();
      return { validated: true };
    });
  }

  /** Every dial, as the Dials view renders them. */
  dials(): Reply {
    const s = this.#session;
    if (!s) return fail(409, "no project is open");
    return attempt(() => ({ dials: core.dials(s.project) }));
  }

  /** Generate a world. */
  generate(seed: string): Reply {
    const s = this.#session;
    if (!s) return fail(409, "no project is open");
    return attempt(() => core.generate(s.project, seed));
  }

  /**
   * May a copied fragment paste here?
   *
   * ⚠ **Checked before anything is open**, because a developer copies from one project and pastes
   * into another — requiring a loaded project would make the common case the awkward one.
   */
  paste(fragment: string, into: string): Reply {
    return attempt(() => ({ allowed: core.mayPaste(fragment, into) }));
  }

  /** Check a state graph, by document text or by a file in the open project. */
  state(rel: string, text?: string): Reply {
    if (text) return attempt(() => core.checkStateGraph(text));
    const s = this.#session;
    if (!s) return fail(409, "no project is open");
    return attempt(() => core.checkStateGraph(s.project.read(rel)));
  }

  /** Read a `.cvcurve` for the curve editor. */
  curves(path: string, text: string): Reply {
    return attempt(() => core.readCurves(path, text));
  }

  /** Read a `.cvunlock` for the table view. */
  unlocks(text: string): Reply {
    return attempt(() => core.readUnlocks(text));
  }

  /** The node palette, grouped for the browser's tree. */
  palette(): Reply {
    return attempt(() => ({
      count: palette().length,
      categories: [...byCategory()].map(([category, nodes]) => ({
        category,
        nodes: nodes.length,
      })),
    }));
  }
}
