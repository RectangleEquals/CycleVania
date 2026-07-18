/**
 * `Held` — the player/party progression state a `Rule` is evaluated against.
 *
 * Generalizes a plain capability Set to also carry **counts** (for multi-key
 * locks like "collect 3 shard-sigils") and **flags** (for switch/event gates).
 * The solver, fill, and simulator all evaluate rules against a `Held`.
 */

import type { Capability } from "./rule.js";

export interface Held {
  /** Does the party hold at least one of `cap`? */
  has(cap: Capability): boolean;
  /** How many of `cap` are held (0 if none) — for counted locks. */
  count(cap: Capability): number;
  /** Is an event/switch flag set? */
  flag(name: string): boolean;
}

/** A concrete, mutable `Held`: capability counts + a flag set. */
export class CapSet implements Held {
  private counts = new Map<Capability, number>();
  private flags = new Set<string>();

  /** Add `n` (may be negative to release an assumed item) of a capability. */
  add(cap: Capability, n = 1): this {
    this.counts.set(cap, (this.counts.get(cap) ?? 0) + n);
    return this;
  }

  setFlag(name: string): this {
    this.flags.add(name);
    return this;
  }

  clearFlag(name: string): this {
    this.flags.delete(name);
    return this;
  }

  has(cap: Capability): boolean {
    return (this.counts.get(cap) ?? 0) > 0;
  }

  count(cap: Capability): number {
    const c = this.counts.get(cap) ?? 0;
    return c > 0 ? c : 0;
  }

  flag(name: string): boolean {
    return this.flags.has(name);
  }

  clone(): CapSet {
    const c = new CapSet();
    c.counts = new Map(this.counts);
    c.flags = new Set(this.flags);
    return c;
  }
}

/** Build a `CapSet` from an iterable of capabilities (each counted once) + flags. */
export function heldOf(caps: Iterable<Capability> = [], flags: Iterable<string> = []): CapSet {
  const c = new CapSet();
  for (const cap of caps) c.add(cap, 1);
  for (const f of flags) c.setFlag(f);
  return c;
}
