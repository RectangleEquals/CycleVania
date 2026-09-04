/**
 * **The service, actually started.**
 *
 * ⚠ **This file exists because two real bugs got past every other test.** The vite proxy pointed at
 * 5174 and the service defaulted to 5173, so the browser could never have reached it; and the
 * entry-point guard compared `import.meta.url` against `` `file://${process.argv[1]}` ``, which on
 * Windows is a native path — so the guard never matched, and `npm run serve` printed nothing and
 * listened on nothing.
 *
 * ▶ **Both were invisible to the round-trip tests**, which import `Api` directly and never bind a
 * socket. A unit test of a server that is never started tests everything except that it is a server.
 *
 * ⚠ **So this binds a real port and makes real requests.** Ephemeral, so it cannot collide with a
 * developer's running editor — a test that fails because the thing it tests is already working is a
 * test people learn to ignore.
 */

import { afterAll, beforeAll, describe, expect, it } from "vitest";
import type { AddressInfo } from "node:net";
import type { Server } from "node:http";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { DEFAULT_PORT, serve } from "../server/serve.ts";

let server: Server;
let base: string;
let scratch: string;

beforeAll(async () => {
  // Port 0 asks the OS for a free one.
  server = serve("127.0.0.1", 0);
  await new Promise<void>((resolve) => server.once("listening", resolve));
  const { port } = server.address() as AddressInfo;
  base = `http://127.0.0.1:${port}`;
  scratch = mkdtempSync(path.join(tmpdir(), "cv-service-"));
});

afterAll(() => {
  server.close();
  rmSync(scratch, { recursive: true, force: true });
});

async function get(route: string): Promise<{ status: number; body: any }> {
  const res = await fetch(`${base}${route}`);
  return { status: res.status, body: await res.json() };
}

async function post(route: string, payload: unknown): Promise<{ status: number; body: any }> {
  const res = await fetch(`${base}${route}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  return { status: res.status, body: await res.json() };
}

describe("the editor's service answers over HTTP", () => {
  it("serves a version, which proves the addon loaded inside a running server", async () => {
    const { status, body } = await get("/api/version");
    expect(status).toBe(200);
    expect(body.version).toMatch(/cyclevania/i);
  });

  it("serves the generated palette", async () => {
    const { body } = await get("/api/palette");
    expect(body.count).toBeGreaterThan(0);
  });

  it("404s an unknown route rather than answering something", async () => {
    const { status } = await get("/api/nope");
    expect(status).toBe(404);
  });

  it("carries a whole project round-trip over the wire", async () => {
    const at = path.join(scratch, "game.cvproj");
    expect((await post("/api/create", { at })).status).toBe(200);

    const authored = "Begin X Id=x Version=1\n   B=2\n   A=1\nEnd X\n";
    const write = await post("/api/write", { rel: "thing.cvs", text: authored });
    expect(write.status).toBe(200);
    // ⚠ The canonical writer normalises, so what lands is not what was sent.
    expect(write.body.written).not.toBe(authored);

    const read = await get("/api/read?rel=thing.cvs");
    expect(read.body.text).toBe(write.body.written);

    // ⚠ A write invalidates; generate must refuse until validate runs again.
    expect((await post("/api/generate", { seed: "s" })).status).toBe(400);
    expect((await post("/api/validate", {})).status).toBe(200);
    const world = await post("/api/generate", { seed: "world-42" });
    expect(world.body.seed).toBe("world-42");

    // The reload, over the wire.
    await post("/api/open", { path: at });
    expect((await get("/api/read?rel=thing.cvs")).body.text).toBe(write.body.written);
  });
});

describe("the wiring the unit tests could not see", () => {
  it("defaults to the port the vite proxy forwards to", () => {
    // ⚠ **The bug this file was written for.** Two constants in two files that must agree, and nothing
    // compared them — so the browser would have reached a closed port with no error anywhere.
    const config = readVite();
    expect(config).toContain(`127.0.0.1:${DEFAULT_PORT}`);
  });
});

function readVite(): string {
  const at = path.resolve(import.meta.dirname, "..", "vite.config.ts");
  // eslint-disable-next-line
  return require("node:fs").readFileSync(at, "utf8") as string;
}
