/**
 * Access logic — boolean rules over capabilities, counted keys, and named world
 * flags. Rules gate mission-graph edges and Locations; the sphere/fill machinery
 * evaluates them to make softlocks impossible by construction. CycleVania never
 * knows what a capability *does* in gameplay — only its id.
 *
 * `always/have/count/flag/not/and/or`. A `flag` may be `volatile` (a timed/
 * one-shot fact): volatile flags are metadata for the solvability layer, which
 * excludes them from the baseline proof — `evalRule` itself treats a volatile
 * flag exactly like any other flag (the caller decides whether it is set).
 */

import type { Held } from "./held.js";

export type CapabilityId = string;

export type Rule =
  | { k: "always" }
  | { k: "have"; cap: CapabilityId }
  | { k: "count"; cap: CapabilityId; n: number }
  | { k: "flag"; name: string; volatile?: boolean }
  | { k: "not"; of: Rule }
  | { k: "and"; of: Rule[] }
  | { k: "or"; of: Rule[] };

export const ALWAYS: Rule = { k: "always" };
export const have = (cap: CapabilityId): Rule => ({ k: "have", cap });
export const count = (cap: CapabilityId, n: number): Rule => ({ k: "count", cap, n });
export const flag = (name: string, opts?: { volatile?: boolean }): Rule =>
  opts?.volatile ? { k: "flag", name, volatile: true } : { k: "flag", name };
export const not = (of: Rule): Rule => ({ k: "not", of });
export const and = (...of: Rule[]): Rule => (of.length === 1 ? (of[0] as Rule) : { k: "and", of });
export const or = (...of: Rule[]): Rule => (of.length === 1 ? (of[0] as Rule) : { k: "or", of });

/** Does `held` satisfy the rule? */
export function evalRule(r: Rule, held: Held): boolean {
  switch (r.k) {
    case "always":
      return true;
    case "have":
      return held.hasCap(r.cap);
    case "count":
      return held.capCount(r.cap) >= r.n;
    case "flag":
      return held.hasFlag(r.name);
    case "not":
      return !evalRule(r.of, held);
    case "and":
      return r.of.every((x) => evalRule(x, held));
    case "or":
      return r.of.some((x) => evalRule(x, held));
  }
}

function collectCaps(r: Rule, out: Set<CapabilityId>): void {
  switch (r.k) {
    case "have":
    case "count":
      out.add(r.cap);
      break;
    case "not":
      collectCaps(r.of, out);
      break;
    case "and":
    case "or":
      for (const x of r.of) collectCaps(x, out);
      break;
    case "always":
    case "flag":
      break;
  }
}

function collectFlags(r: Rule, out: Set<string>): void {
  switch (r.k) {
    case "flag":
      out.add(r.name);
      break;
    case "not":
      collectFlags(r.of, out);
      break;
    case "and":
    case "or":
      for (const x of r.of) collectFlags(x, out);
      break;
    case "always":
    case "have":
    case "count":
      break;
  }
}

/** Every capability a rule references, deduped, in first-occurrence order. */
export function ruleCaps(r: Rule): CapabilityId[] {
  const out = new Set<CapabilityId>();
  collectCaps(r, out);
  return [...out];
}

/** Every flag a rule references, deduped, in first-occurrence order. */
export function ruleFlags(r: Rule): string[] {
  const out = new Set<string>();
  collectFlags(r, out);
  return [...out];
}

/**
 * The subset of a rule's caps still needed to satisfy it, given `held`. An
 * already-satisfied rule needs nothing (`[]`). Otherwise every unmet cap in the
 * rule is returned — an A∧B door reports BOTH missing caps (never collapsed to
 * one "primary"), and an unmet A∨B reports both alternatives. Flags are not caps,
 * so a `flag` requirement never appears here.
 */
export function missingCaps(r: Rule, held: Held): CapabilityId[] {
  if (evalRule(r, held)) return [];
  return ruleCaps(r).filter((cap) => !held.hasCap(cap));
}

/** True if any `flag` node anywhere in the rule is volatile. */
export function usesVolatileFlag(r: Rule): boolean {
  switch (r.k) {
    case "flag":
      return r.volatile === true;
    case "not":
      return usesVolatileFlag(r.of);
    case "and":
    case "or":
      return r.of.some(usesVolatileFlag);
    default:
      return false;
  }
}

export const isOpen = (r: Rule): boolean => r.k === "always";
