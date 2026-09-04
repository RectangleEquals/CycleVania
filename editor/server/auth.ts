/**
 * **Who may reach the editor's service.**
 *
 * ⚠ **Auth is the editor's, because the service is the editor's.** There is no CycleVania service to
 * secure — the core is a function call away, in-process. This guards the editor's own HTTP surface.
 *
 * # Loopback needs nothing
 *
 * ⚠ **A process that can reach `127.0.0.1` can already read the project off disk.** Asking it to pair
 * first would be a ritual, not a control: it protects nothing the filesystem is not already handing
 * over, and a ritual with no teeth trains people to disable the thing that does have them.
 *
 * # Beyond loopback, a short-lived single-use code
 *
 * ⚠ **And the failure mode of an unconfigured editor must be refusal, not exposure.** An editor bound
 * to a LAN address with no pairing configured does not start. That is the whole design: the mistake a
 * tired developer makes at 2am is *forgetting* to configure something, so forgetting must fail closed.
 *
 * ▶ **Deliberately absent: accounts, roles, TLS termination, and any notion of a remote project.** The
 * editor serves one project on one machine's disk. A team wanting shared editing wants version control,
 * which they already have.
 */

import { randomBytes, timingSafeEqual } from "node:crypto";

/** How long a pairing code is good for. Short, because it is handed over out of band. */
export const PAIRING_TTL_MS = 5 * 60 * 1000;

/** Is this the machine talking to itself? */
export function isLoopback(address: string | undefined): boolean {
  if (!address) return false;
  const a = address.replace(/^::ffff:/, "");
  return a === "127.0.0.1" || a === "::1" || a.startsWith("127.");
}

/** A code handed to one device, once. */
interface Pairing {
  code: string;
  expires: number;
  used: boolean;
}

/**
 * The editor's pairing state.
 *
 * ⚠ **Single-use and expiring, both.** Single-use alone leaves a code valid forever until someone
 * happens to use it; expiring alone leaves it replayable for its whole life. Neither property covers
 * the other's gap.
 */
export class Pairings {
  #issued: Pairing[] = [];
  #now: () => number;

  constructor(now: () => number = Date.now) {
    this.#now = now;
  }

  /** Issue a code for one device. */
  issue(): string {
    const code = randomBytes(4).toString("hex").toUpperCase();
    this.#issued.push({ code, expires: this.#now() + PAIRING_TTL_MS, used: false });
    return code;
  }

  /** How many codes could still be redeemed. */
  get outstanding(): number {
    return this.#issued.filter((p) => !p.used && p.expires > this.#now()).length;
  }

  /**
   * Redeem a code. True once, false ever after.
   *
   * ⚠ **Compared in constant time.** A pairing code is short enough that a timing oracle is a real way
   * to walk it out one character at a time, and `===` on strings leaks exactly that.
   */
  redeem(offered: string): boolean {
    const now = this.#now();
    for (const p of this.#issued) {
      if (p.used || p.expires <= now) continue;
      const a = Buffer.from(p.code);
      const b = Buffer.from(offered);
      if (a.length === b.length && timingSafeEqual(a, b)) {
        p.used = true;
        return true;
      }
    }
    return false;
  }
}

/** What the service decided about one request. */
export type Verdict = { allowed: true } | { allowed: false; reason: string };

/**
 * May this request be served?
 *
 * `paired` is the set of device tokens that have already redeemed a code — a browser keeps one and
 * sends it back, so pairing happens once per device rather than once per request.
 */
export function admit(
  remote: string | undefined,
  token: string | undefined,
  paired: Set<string>,
): Verdict {
  if (isLoopback(remote)) return { allowed: true };
  if (token && paired.has(token)) return { allowed: true };
  return {
    allowed: false,
    reason: "this device is not paired — pair it from the editor on the host machine",
  };
}

/**
 * Refuse to start rather than serve a LAN with no way to pair.
 *
 * ⚠ **Checked at startup, not per request.** A per-request check would let the service come up, look
 * healthy, and reject everything — which reads as a bug in the client rather than a missing setting.
 */
export function startupRefusal(host: string, pairingConfigured: boolean): string | undefined {
  if (isLoopback(host) || host === "localhost") return undefined;
  if (pairingConfigured) return undefined;
  return (
    `refusing to serve ${host}: no pairing is configured, and an editor reachable from the network ` +
    `without one would hand a project to anyone who found the port. Bind to 127.0.0.1, or configure ` +
    `pairing.`
  );
}
