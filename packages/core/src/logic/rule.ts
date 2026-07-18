/**
 * Access logic — boolean rules over CAPABILITIES (base gadget capabilities, key
 * items, counted keys, and virtual event flags). These gate region/room edges;
 * the sphere/fill machinery uses them to make softlocks impossible by
 * construction. Class/build abilities never appear here (any-party guarantee).
 *
 * Extends the classic always/have/and/or with `count` (multi-key), `flag`
 * (switch/event), and `not`. Rules are evaluated against a `Held` (held.ts).
 */

import type { Held } from "./held.js";

export type Capability = string;

export type Rule =
  | { k: "always" }
  | { k: "have"; cap: Capability }
  | { k: "count"; cap: Capability; n: number }
  | { k: "flag"; name: string }
  | { k: "not"; of: Rule }
  | { k: "and"; of: Rule[] }
  | { k: "or"; of: Rule[] };

export const ALWAYS: Rule = { k: "always" };
export const have = (cap: Capability): Rule => ({ k: "have", cap });
export const count = (cap: Capability, n: number): Rule => ({ k: "count", cap, n });
export const flag = (name: string): Rule => ({ k: "flag", name });
export const not = (of: Rule): Rule => ({ k: "not", of });
export const and = (...of: Rule[]): Rule => (of.length === 1 ? (of[0] as Rule) : { k: "and", of });
export const or = (...of: Rule[]): Rule => (of.length === 1 ? (of[0] as Rule) : { k: "or", of });

/** Does `held` satisfy the rule? */
export function evalRule(r: Rule, held: Held): boolean {
  switch (r.k) {
    case "always":
      return true;
    case "have":
      return held.has(r.cap);
    case "count":
      return held.count(r.cap) >= r.n;
    case "flag":
      return held.flag(r.name);
    case "not":
      return !evalRule(r.of, held);
    case "and":
      return r.of.every((x) => evalRule(x, held));
    case "or":
      return r.of.some((x) => evalRule(x, held));
  }
}

/** All capabilities a rule references (for validation / UI / remembered locks). */
export function ruleCaps(r: Rule, out = new Set<Capability>()): Set<Capability> {
  switch (r.k) {
    case "have":
    case "count":
      out.add(r.cap);
      break;
    case "not":
      ruleCaps(r.of, out);
      break;
    case "and":
    case "or":
      for (const x of r.of) ruleCaps(x, out);
      break;
    case "always":
    case "flag":
      break;
  }
  return out;
}

/** All event-flags a rule references. */
export function ruleFlags(r: Rule, out = new Set<string>()): Set<string> {
  switch (r.k) {
    case "flag":
      out.add(r.name);
      break;
    case "not":
      ruleFlags(r.of, out);
      break;
    case "and":
    case "or":
      for (const x of r.of) ruleFlags(x, out);
      break;
    default:
      break;
  }
  return out;
}

/**
 * The subset of `caps` a rule still needs given what's `held` — used by the
 * Astrolabe "remembered locks" UI to tell the player which gadget is missing
 * for a compound gate (never collapsing A∧B to a single "primary" cap).
 */
export function missingCaps(r: Rule, held: Held): Set<Capability> {
  const out = new Set<Capability>();
  for (const cap of ruleCaps(r)) if (!held.has(cap)) out.add(cap);
  return out;
}

export const isOpen = (r: Rule): boolean => r.k === "always";
