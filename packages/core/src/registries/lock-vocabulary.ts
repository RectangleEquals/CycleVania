/**
 * Lock vocabulary — named locks as data. A lock is either a bare rule-builder
 * (just a gate) or a `ChallengeTemplate` that also names a spatial `recipe` (the
 * challenge half of the affordance/challenge contract). The `solvedBy` rule is
 * evaluated to a concrete `Rule` at registry-build time and is the SAME rule the
 * solver uses on the edge — geometry and logic can never diverge.
 */

import { ALWAYS, and, count, flag, have, not, or, type Rule } from "../logic/index.js";
import type { Traversal } from "../types.js";

export interface RuleDsl {
  always: Rule;
  have: (cap: string) => Rule;
  count: (cap: string, n: number) => Rule;
  flag: (name: string) => Rule;
  not: (of: Rule) => Rule;
  and: (...of: Rule[]) => Rule;
  or: (...of: Rule[]) => Rule;
}

export const RULE_DSL: RuleDsl = { always: ALWAYS, have, count, flag, not, and, or };

export type RuleBuilder = (r: RuleDsl) => Rule;

export interface ChallengeTemplate {
  id?: string;
  /** The gate — identical to the rule the solver stamps on the edge. */
  solvedBy: RuleBuilder;
  /** The spatial recipe archetype id that physically expresses the lock. */
  recipe: string;
  placement?: { roomTypes?: string[]; minCells?: [number, number, number]; biomeAffinity?: string[] };
  difficulty?: { base: number; scaleWithDepth?: number };
  /** Using the answer mutates the map (opens a drop / sets a flag). */
  sideEffects?: { addsEdge?: { traversal: Traversal; oneWay: boolean }; setsFlag?: string };
}

export type LockDef = RuleBuilder | ChallengeTemplate;
export type LockVocabularyInput = Record<string, LockDef>;

/** A lock resolved to a concrete rule + its recipe (what the composer consumes). */
export interface ResolvedLock {
  name: string;
  rule: Rule;
  recipe: string;
  def: ChallengeTemplate;
}
