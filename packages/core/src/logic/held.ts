/**
 * `Held` — the progression state a `Rule` is evaluated against: capability counts
 * (for level/multi-key locks) + a set of world flags. The solver, fill, and
 * simulator all evaluate rules against a `Held`.
 *
 * Method names are `hasCap`/`capCount`/`hasFlag` (distinct from the `flag()`
 * rule-builder, which shares the bare word "flag"). `CapSet` is the concrete,
 * enumerable, cloneable implementation; `HeldData` is its serializable form.
 */

import type { CapabilityId } from "./rule.js";

export interface Held {
  /** Does the party hold at least one of `cap`? */
  hasCap(cap: CapabilityId): boolean;
  /** How many of `cap` are held (0 if none) — for counted/level locks. */
  capCount(cap: CapabilityId): number;
  /** Is a world flag set? */
  hasFlag(name: string): boolean;
}

/** The plain, serializable form of a `Held`. */
export interface HeldData {
  caps: Record<CapabilityId, number>;
  flags: string[];
}

/** A concrete, mutable, enumerable `Held`: capability counts + a flag set. */
export class CapSet implements Held {
  private counts = new Map<CapabilityId, number>();
  private flags = new Set<string>();

  /** Add `n` (may be negative to release an assumed item) of a capability. */
  add(cap: CapabilityId, n = 1): this {
    this.counts.set(cap, (this.counts.get(cap) ?? 0) + n);
    return this;
  }

  addFlag(name: string): this {
    this.flags.add(name);
    return this;
  }

  removeFlag(name: string): this {
    this.flags.delete(name);
    return this;
  }

  hasCap(cap: CapabilityId): boolean {
    return (this.counts.get(cap) ?? 0) > 0;
  }

  capCount(cap: CapabilityId): number {
    const c = this.counts.get(cap) ?? 0;
    return c > 0 ? c : 0;
  }

  hasFlag(name: string): boolean {
    return this.flags.has(name);
  }

  /** The capability ids currently held (count > 0), in insertion order. */
  capIds(): CapabilityId[] {
    const out: CapabilityId[] = [];
    for (const [cap, n] of this.counts) if (n > 0) out.push(cap);
    return out;
  }

  /** The flags currently set, in insertion order. */
  flagNames(): string[] {
    return [...this.flags];
  }

  clone(): CapSet {
    const c = new CapSet();
    c.counts = new Map(this.counts);
    c.flags = new Set(this.flags);
    return c;
  }

  /** Serializable snapshot with keys in deterministic (sorted) order. */
  toData(): HeldData {
    const caps: Record<CapabilityId, number> = {};
    for (const key of [...this.counts.keys()].sort()) {
      const v = this.counts.get(key) ?? 0;
      if (v > 0) caps[key] = v;
    }
    return { caps, flags: [...this.flags].sort() };
  }
}

/** Build a `CapSet` from capabilities (each counted once) + flags. */
export function heldOf(caps: Iterable<CapabilityId> = [], flags: Iterable<string> = []): CapSet {
  const c = new CapSet();
  for (const cap of caps) c.add(cap, 1);
  for (const f of flags) c.addFlag(f);
  return c;
}

/** Rebuild a `CapSet` from its serializable form. */
export function heldFromData(data: HeldData): CapSet {
  const c = new CapSet();
  for (const [cap, n] of Object.entries(data.caps)) if (n > 0) c.add(cap, n);
  for (const f of data.flags) c.addFlag(f);
  return c;
}
