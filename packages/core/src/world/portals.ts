/**
 * Cross-Reach navigation topology — deliberately NOT part of any Reach's
 * solvability. A portal only says two ALREADY-REALIZED Reaches are physically
 * connected; never that reaching one requires anything from the other.
 */

import type { Rule } from "../logic/index.js";

export interface ReachPortal {
  fromReach: number;
  toReach: number;
  oneWay: boolean;
  fromSpaceHint: string;
  toSpaceHint: string;
  /** Evaluated by the HOST's runtime for traversal UI, never by the solver. */
  gate?: Rule;
}
