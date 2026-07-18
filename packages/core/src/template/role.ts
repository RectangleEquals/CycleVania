/**
 * Region roles — the vocabulary a Reach template is built from. Roles are
 * game-agnostic: a "hub" is CrawlStar's Sanctum or Metroid Prime's save room; a
 * "capstone" is a boss/set-piece; a "segment" is critical-path progress.
 */

export type RegionRole =
  | "hub" // start/save region; over-provisions bootstrap slots (fill safety)
  | "segment" // linear-progress region on the critical path
  | "gate" // a region whose entrance edge is the primary lock
  | "vault" // optional branch (loot/secret), off the critical path
  | "capstone" // boss / set-piece region
  | "terminal"; // exit / next-hub handoff
