/**
 * **M18's green condition** — a hook graph is authored, saved, reloaded and compiled, and a fragment
 * pasted from a spine into a schematic is refused with a reason.
 *
 * ⚠ **Through the real addon, end to end.** The rules in `graph.test.ts` are local logic; this is the
 * part that only works if the whole seam does — the canonical writer, the compiler, and the format
 * check all agreeing about one document.
 */

import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Api } from "../server/api.ts";
import { mayPaste } from "../server/bindings.ts";

function scratch(): string {
  return mkdtempSync(path.join(tmpdir(), "cv-authoring-"));
}

/** A hook graph, shaped the way the palette would emit it. */
const HOOK = `Begin Schematic Version=1 Path=/Content/Items/Hookshot Extends=Kind'/Core/Item' Id=s
   Begin Graph Name="requires" Role=Hook Id=grf
      Begin Node Id=n_0001 Op=core.instances_of Pos=(0,0)
         Pin (Name=out, Dir=Out, Type=bool, To=(n_0002.cond))
      End Node
      Begin Node Id=n_0002 Op=core.branch Pos=(80,0)
      End Node
   End Graph
End Schematic
`;

describe("a hook graph survives the whole round trip", () => {
  it("is authored, saved, reloaded and compiled", () => {
    const dir = scratch();
    const at = path.join(dir, "game.cvproj");
    const api = new Api();
    api.create(at);

    const write = api.write("hookshot.cvs", HOOK);
    expect(write.status).toBe(200);
    const written = (write.body as { written: string }).written;

    // Reloaded: a fresh session, a fresh open, the same bytes.
    const reopened = new Api();
    reopened.open(at);
    expect((reopened.read("hookshot.cvs").body as { text: string }).text).toBe(written);

    // ⚠ Compiled — `validate` runs the graph through `cv-compile`, so this is the assertion that
    // would have passed for four milestones while compiling nothing.
    expect(reopened.validate().status).toBe(200);
    rmSync(dir, { recursive: true, force: true });
  });

  it("refuses to save a graph the compiler rejects, naming the file", () => {
    const dir = scratch();
    const at = path.join(dir, "game.cvproj");
    const api = new Api();
    api.create(at);
    api.write("broken.cvs", HOOK.replace("core.instances_of", "array.is_emty"));

    const verdict = api.validate();
    expect(verdict.status).toBe(400);
    expect((verdict.body as { error: string }).error).toContain("broken.cvs");
    rmSync(dir, { recursive: true, force: true });
  });
});

describe("P06 — paste is format-scoped", () => {
  const fragment = (format: string) =>
    `Begin Fragment Version=1 Format=${format} Source=/Content/x\n` +
    `   Begin Node Id=n_0001 Op=core.branch Pos=(0,0)\n   End Node\nEnd Fragment\n`;

  it("accepts a schematic fragment into a schematic", () => {
    expect(mayPaste(fragment("Schematic"), "Schematic")).toBe(true);
  });

  it("refuses a spine fragment into a schematic, with a reason naming both", () => {
    // ⚠ **Both spellings parse**, which is what makes the check necessary rather than pedantic: a
    // `core.` op inside a spine is a perfectly valid CVB document that means nothing.
    let message = "";
    try {
      mayPaste(fragment("Spine"), "Schematic");
    } catch (e) {
      message = (e as Error).message;
    }
    expect(message).not.toBe("");
    expect(message.toLowerCase()).toContain("spine");
    expect(message.toLowerCase()).toContain("schematic");
  });

  it("tells a format mismatch and a syntax error apart", () => {
    // ⚠ **A valid document of the wrong format is not a parse failure.** Both were reported as
    // "did not parse", which sends a developer looking for a syntax error that is not there.
    let mismatch = "";
    try {
      mayPaste(fragment("Spine"), "Schematic");
    } catch (e) {
      mismatch = (e as Error).message;
    }
    let malformed = "";
    try {
      mayPaste("Begin Unclosed\n", "Schematic");
    } catch (e) {
      malformed = (e as Error).message;
    }
    expect(mismatch).not.toContain("did not parse");
    expect(malformed).toContain("did not parse");
  });

  it("refuses a fragment that does not say where it came from", () => {
    let message = "";
    try {
      mayPaste("Begin Fragment Version=1 Source=/Content/x\nEnd Fragment\n", "Schematic");
    } catch (e) {
      message = (e as Error).message;
    }
    expect(message).not.toBe("");
  });

  it("answers through the editor's route as well as the binding", () => {
    const api = new Api();
    expect(api.paste(fragment("Schematic"), "Schematic").status).toBe(200);
    expect(api.paste(fragment("Spine"), "Schematic").status).toBe(400);
  });
});

describe("the dial line carries what the panel renders", () => {
  it("is JSON with a doc and bounds, not six tab-separated fields", () => {
    // ⚠ The plan names five things the panel renders — kind, default, bounds, doc, effective — and the
    // binding used to carry three of them. A doc containing a tab would also have split the row.
    const dir = scratch();
    const at = path.join(dir, "game.cvproj");
    const api = new Api();
    api.create(at);
    const reply = api.dials();
    expect(reply.status).toBe(200);
    // An empty project declares none; the shape is what matters, and it is asserted in Rust where the
    // fixture can declare one. Here: the route works and returns a list.
    expect(Array.isArray((reply.body as { dials: unknown[] }).dials)).toBe(true);
    rmSync(dir, { recursive: true, force: true });
  });
});
