/**
 * **M16's green condition** — the editor opens a project, lists its content, and saves a change that
 * survives a reload byte-identically.
 *
 * ⚠ **Through the bindings, in-process, with no service running.** These tests import the API object
 * directly. If any of them needed a server to be up, the editor would have a runtime dependency the
 * design says it must not, and the failure would show up here rather than in production.
 */

import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Api } from "../server/api.ts";

function scratch(): string {
  return mkdtempSync(path.join(tmpdir(), "cv-editor-"));
}

/** Deliberately not canonical: keys out of order, so a normalising write is visible. */
const AUTHORED = "Begin X Id=x Version=1\n   B=2\n   A=1\nEnd X\n";

describe("the editor reaches the core with nothing in between", () => {
  it("loads the addon and answers a version", () => {
    const reply = new Api().version();
    expect(reply.status).toBe(200);
    expect((reply.body as { version: string }).version).toMatch(/cyclevania/i);
  });

  it("creates a project, lists it empty, and opens it again", () => {
    const dir = scratch();
    const at = path.join(dir, "game.cvproj");
    const api = new Api();

    const made = api.create(at);
    expect(made.status).toBe(200);
    expect((made.body as { content: string[] }).content).toEqual([]);

    const reopened = new Api().open(at);
    expect(reopened.status).toBe(200);
    rmSync(dir, { recursive: true, force: true });
  });

  it("saves a change that survives a reload byte-identically", () => {
    const dir = scratch();
    const at = path.join(dir, "game.cvproj");
    const api = new Api();
    api.create(at);

    const write = api.write("thing.cvs", AUTHORED);
    expect(write.status).toBe(200);
    const written = (write.body as { written: string }).written;

    // ⚠ The canonical writer reorders header keys and sorts members, so what lands is *not* what was
    // sent — an editor assuming otherwise would show a buffer that disagrees with the file.
    expect(written).not.toBe(AUTHORED);

    expect((api.read("thing.cvs").body as { text: string }).text).toBe(written);
    expect((api.content().body as { content: string[] }).content).toEqual(["thing.cvs"]);

    // The reload: a different Api, a fresh open, the same bytes.
    const fresh = new Api();
    fresh.open(at);
    expect((fresh.read("thing.cvs").body as { text: string }).text).toBe(written);

    // ⚠ And writing the canonical form back is a no-op, which is what makes "byte-identical" a
    // property rather than a coincidence of this one sample.
    const again = fresh.write("thing.cvs", written);
    expect((again.body as { written: string }).written).toBe(written);
    rmSync(dir, { recursive: true, force: true });
  });

  it("refuses to generate before validating, then generates", () => {
    const dir = scratch();
    const at = path.join(dir, "game.cvproj");
    const api = new Api();
    api.create(at);
    api.write("thing.cvs", AUTHORED);

    // ⚠ A write invalidates: the tree changed, so the last validate describes something that is gone.
    expect(api.generate("s").status).toBe(400);
    expect(api.validate().status).toBe(200);

    const world = api.generate("world-42");
    expect(world.status).toBe(200);
    expect((world.body as { seed: string }).seed).toBe("world-42");
    rmSync(dir, { recursive: true, force: true });
  });

  it("creates from an existing project by copying, not linking", () => {
    const dir = scratch();
    const presetAt = path.join(dir, "preset.cvproj");
    const preset = new Api();
    preset.create(presetAt);
    preset.write("a.cvs", "Begin X Id=x Version=1\nEnd X\n");

    const madeAt = path.join(dir, "made", "game.cvproj");
    const made = new Api();
    made.create(madeAt, presetAt);
    expect((made.content().body as { content: string[] }).content).toEqual(["a.cvs"]);

    made.write("a.cvs", "Begin X Id=x Version=2\nEnd X\n");
    const source = new Api();
    source.open(presetAt);
    expect((source.read("a.cvs").body as { text: string }).text).toContain("Version=1");
    rmSync(dir, { recursive: true, force: true });
  });

  it("says so when nothing is open, rather than inventing an empty project", () => {
    // ⚠ 409, not an empty list. "No project open" and "an open project with no files" are different
    // states, and an editor that conflates them shows a blank browser for a bug.
    const api = new Api();
    expect(api.content().status).toBe(409);
    expect(api.read("x.cvs").status).toBe(409);
    expect(api.generate("s").status).toBe(409);
  });
});

describe("the palette is read, never computed", () => {
  it("loads the generated palette and groups it", () => {
    const reply = new Api().palette();
    expect(reply.status).toBe(200);
    const body = reply.body as { count: number; categories: unknown[] };
    expect(body.count).toBeGreaterThan(0);
    expect(body.categories.length).toBeGreaterThan(0);
  });
});
