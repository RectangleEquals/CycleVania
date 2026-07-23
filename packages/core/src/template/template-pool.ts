/** Draw a depth-scoped template from a pool with seeded weights. */

import { GenError } from "../errors.js";
import type { Rng } from "../math/index.js";
import type { ReachTemplate, ReachTemplatePool } from "./reach-template.js";

export function drawTemplate(pool: ReachTemplatePool, depth: number, rng: Rng): ReachTemplate {
  const entries = pool.poolAt(depth);
  if (entries.length === 0) throw new GenError("template.empty-pool", `template pool is empty at depth ${depth}`, { depth });
  return rng.weighted(entries.map((e) => ({ item: e.template, weight: e.weight })));
}
