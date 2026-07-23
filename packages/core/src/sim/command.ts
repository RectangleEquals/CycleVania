import type { CapabilityId } from "../logic/index.js";
import type { RegionId } from "../graph/index.js";

export type Command =
  | { k: "move"; to: RegionId } // one step to an adjacent, open region
  | { k: "goto"; to: RegionId } // pathfind via currently-open links, then move
  | { k: "take" } // collect the current region's uncollected Locations
  | { k: "use"; itemId: string }
  | { k: "interact"; puzzleId: string }
  | { k: "see" }
  | { k: "why"; to: RegionId }
  | { k: "give"; cap: CapabilityId }
  | { k: "reset" }
  | { k: "solve" };
