/**
 * **The editor's HTTP service.**
 *
 * ⚠ **It exists because a browser cannot load a native Node addon**, not because CycleVania needs a
 * server. The core is in-process on this side of it; a host project embeds the same addon and needs
 * none of this.
 *
 * ▶ **Started by the editor, owned by the editor, secured by the editor.** See `auth.ts` for why
 * loopback needs no pairing and why a LAN bind with no pairing configured refuses to start.
 */

import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import { randomBytes } from "node:crypto";
import { pathToFileURL } from "node:url";
import { Api } from "./api.ts";
import { Pairings, admit, startupRefusal } from "./auth.ts";

const api = new Api();
const pairings = new Pairings();
const paired = new Set<string>();

/** Read a JSON body, or `{}`. */
async function body(req: IncomingMessage): Promise<Record<string, string>> {
  const chunks: Buffer[] = [];
  for await (const c of req) chunks.push(c as Buffer);
  if (chunks.length === 0) return {};
  try {
    return JSON.parse(Buffer.concat(chunks).toString("utf8")) as Record<string, string>;
  } catch {
    return {};
  }
}

/** Route one request. */
async function route(req: IncomingMessage, url: URL): Promise<{ status: number; body: unknown }> {
  const b = await body(req);
  const q = (name: string): string => b[name] ?? url.searchParams.get(name) ?? "";

  switch (`${req.method} ${url.pathname}`) {
    case "GET /api/version":
      return api.version();
    case "GET /api/palette":
      return api.palette();
    case "POST /api/open":
      return api.open(q("path"));
    case "POST /api/create":
      return api.create(q("at"), q("from") || undefined);
    case "GET /api/content":
      return api.content();
    case "GET /api/read":
      return api.read(q("rel"));
    case "POST /api/write":
      return api.write(q("rel"), q("text"));
    case "POST /api/validate":
      return api.validate();
    case "GET /api/dials":
      return api.dials();
    case "POST /api/generate":
      return api.generate(q("seed"));
    case "POST /api/paste":
      return api.paste(q("fragment"), q("into"));
    default:
      return { status: 404, body: { error: `no route ${req.method} ${url.pathname}` } };
  }
}

/** The API port. ⚠ **5174, not 5173** — vite's dev server owns 5173 and proxies `/api` here. */
export const DEFAULT_PORT = 5174;

/** Start the service. */
export function serve(host = "127.0.0.1", port = DEFAULT_PORT): ReturnType<typeof createServer> {
  const refusal = startupRefusal(host, pairings.outstanding > 0);
  if (refusal) {
    // ⚠ Thrown, not logged. A service that warns and serves anyway has warned nobody.
    throw new Error(refusal);
  }

  const server = createServer((req: IncomingMessage, res: ServerResponse) => {
    void (async () => {
      const url = new URL(req.url ?? "/", `http://${req.headers.host ?? host}`);

      // Pairing is the one route an unpaired device may reach.
      if (req.method === "POST" && url.pathname === "/api/pair") {
        const offered = (await body(req)).code ?? "";
        if (pairings.redeem(offered)) {
          const token = randomBytes(16).toString("hex");
          paired.add(token);
          return send(res, 200, { token });
        }
        return send(res, 403, { error: "that code is not valid" });
      }

      const verdict = admit(
        req.socket.remoteAddress,
        req.headers["x-cyclevania-device"] as string | undefined,
        paired,
      );
      if (!verdict.allowed) return send(res, 403, { error: verdict.reason });

      const reply = await route(req, url);
      send(res, reply.status, reply.body);
    })();
  });

  server.listen(port, host);
  return server;
}

function send(res: ServerResponse, status: number, payload: unknown): void {
  const text = JSON.stringify(payload);
  res.writeHead(status, {
    "content-type": "application/json",
    "content-length": Buffer.byteLength(text),
  });
  res.end(text);
}

/** Issue a pairing code, for the editor UI to display. */
export function pair(): string {
  return pairings.issue();
}

// ⚠ **Compared as URLs, not by string-splicing one.** `process.argv[1]` is a native path — on Windows
// `E:\…\serve.ts` — and `file://` + that is not the `file:///E:/…` form `import.meta.url` carries. The
// naive comparison silently never matches, so the service starts, prints nothing, and listens on nothing.
const invokedDirectly =
  process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href;

if (invokedDirectly) {
  const host = process.env["CV_EDITOR_HOST"] ?? "127.0.0.1";
  const port = Number(process.env["CV_EDITOR_PORT"] ?? DEFAULT_PORT);
  serve(host, port);
  console.log(`editor service on http://${host}:${port}`);
}
