/**
 * **P05 — who may reach the editor's service.**
 *
 * ⚠ **The test that matters is the refusal.** An editor that serves a LAN with no pairing configured is
 * a project handed to whoever finds the port, and *"we meant to configure it"* is the most common way
 * that happens. So forgetting must fail closed, and this asserts it does.
 */

import { describe, expect, it } from "vitest";
import { Pairings, admit, isLoopback, startupRefusal, PAIRING_TTL_MS } from "../server/auth.ts";

describe("loopback needs no pairing", () => {
  it("recognises the machine talking to itself, in both address families", () => {
    for (const a of ["127.0.0.1", "::1", "::ffff:127.0.0.1", "127.0.1.1"]) {
      expect(isLoopback(a), a).toBe(true);
    }
    for (const a of ["192.168.1.9", "10.0.0.4", "", undefined]) {
      expect(isLoopback(a), String(a)).toBe(false);
    }
  });

  it("admits loopback with no token at all", () => {
    // ⚠ A process that can reach 127.0.0.1 can already read the project off disk. Pairing it would be
    // a ritual, and a ritual with no teeth trains people to disable the controls that have them.
    expect(admit("127.0.0.1", undefined, new Set()).allowed).toBe(true);
  });

  it("refuses an unpaired device from the network", () => {
    const verdict = admit("192.168.1.9", undefined, new Set());
    expect(verdict.allowed).toBe(false);
  });

  it("admits a device that has already paired", () => {
    expect(admit("192.168.1.9", "token-a", new Set(["token-a"])).allowed).toBe(true);
    expect(admit("192.168.1.9", "token-b", new Set(["token-a"])).allowed).toBe(false);
  });
});

describe("a pairing code is single-use and short-lived", () => {
  it("redeems once and never again", () => {
    const p = new Pairings();
    const code = p.issue();
    expect(p.redeem(code)).toBe(true);
    expect(p.redeem(code)).toBe(false);
  });

  it("expires even if nobody uses it", () => {
    // ⚠ Single-use alone leaves a code valid forever until someone happens to use it.
    let now = 1_000;
    const p = new Pairings(() => now);
    const code = p.issue();
    now += PAIRING_TTL_MS + 1;
    expect(p.redeem(code)).toBe(false);
    expect(p.outstanding).toBe(0);
  });

  it("rejects a wrong code without saying how wrong", () => {
    const p = new Pairings();
    p.issue();
    expect(p.redeem("00000000")).toBe(false);
    expect(p.redeem("")).toBe(false);
  });
});

describe("an unconfigured editor refuses to serve rather than exposing a project", () => {
  it("starts on loopback with no pairing", () => {
    expect(startupRefusal("127.0.0.1", false)).toBeUndefined();
    expect(startupRefusal("localhost", false)).toBeUndefined();
  });

  it("refuses a LAN bind with no pairing configured", () => {
    // ⚠ This is the whole of P05. The mistake is *forgetting*, so forgetting fails closed.
    const refusal = startupRefusal("0.0.0.0", false);
    expect(refusal).toBeDefined();
    expect(refusal).toContain("no pairing is configured");
  });

  it("starts on a LAN bind once pairing exists", () => {
    expect(startupRefusal("0.0.0.0", true)).toBeUndefined();
  });
});
