/**
 * ReachComposer — holds all Areas of a Reach. It runs the grammar (region graph +
 * solvable placement), assigns an area per region, decides each area's world lane
 * and depth/biome, wires portals from the region edges (carrying their gates),
 * and delegates each area's interior to the AreaComposer.
 */

import { boxUnionAll } from "../math/geom.js";
import { ruleCaps, type Capability } from "../logic/index.js";
import { generateReach, type GeneratedReach } from "../template/grammar.js";
import type { ReachTemplate } from "../template/template.js";
import type { RegionId } from "../graph/region-graph.js";
import type { AreaDescriptor, AreaLink, ReachDescriptor } from "../descriptors/descriptor.js";
import { composeArea, type PortalRequest } from "./area-composer.js";
import type { ComposeContext } from "./context.js";

export interface ComposeReachOptions {
  template: ReachTemplate;
  reachIndex?: number;
  /** Complexity depth; defaults to a ramp from reachIndex. */
  depth?: number;
  startCaps?: Iterable<Capability>;
  styleId?: string;
}

export interface ReachResult {
  reach: GeneratedReach;
  descriptor: ReachDescriptor;
}

const LANE = 40; // world Y spacing between area lanes

export function composeReach(ctx: ComposeContext, opts: ComposeReachOptions): ReachResult {
  const reachIndex = opts.reachIndex ?? 0;
  const depth = opts.depth ?? reachIndex * 8;
  const styleId = opts.styleId ?? [...ctx.registry.styles.keys()][0] ?? "default";
  const reachSeed = `${ctx.seed}:reach${reachIndex}`;

  const reach = generateReach({
    seed: reachSeed,
    template: opts.template,
    items: ctx.registry.items.progression,
    ...(opts.startCaps ? { startCaps: opts.startCaps } : {}),
  });

  // area ids by forward BFS from the start region (host before children)
  const order: RegionId[] = [reach.graph.start];
  const seen = new Set<RegionId>([reach.graph.start]);
  for (let h = 0; h < order.length; h++) {
    const u = order[h] as RegionId;
    for (const e of reach.graph.edges) if (e.from === u && !seen.has(e.to)) (seen.add(e.to), order.push(e.to));
  }
  for (const r of reach.graph.regions) if (!seen.has(r)) order.push(r);
  const areaIdOf = new Map<RegionId, number>();
  order.forEach((r, i) => areaIdOf.set(r, i + 1));

  const itemCap = new Map<string, Capability | undefined>();
  for (const it of reach.items) itemCap.set(it.id, it.grants);

  // portals + area links from region edges
  const portalsByRegion = new Map<RegionId, PortalRequest[]>();
  const pushPortal = (r: RegionId, pr: PortalRequest): void => {
    const a = portalsByRegion.get(r) ?? [];
    a.push(pr);
    portalsByRegion.set(r, a);
  };
  const links: AreaLink[] = [];
  reach.graph.edges.forEach((e, i) => {
    const key = `e${i}`;
    const oneWay = e.oneWay ? { oneWay: true as const } : {};
    pushPortal(e.from, { key, kind: "exit", edge: { from: e.from, to: e.to }, requires: e.rule, ...oneWay });
    pushPortal(e.to, { key, kind: "entry", edge: { from: e.from, to: e.to }, requires: e.rule, ...oneWay });
    links.push({
      fromAreaId: areaIdOf.get(e.from) as number,
      toAreaId: areaIdOf.get(e.to) as number,
      requires: e.rule,
      requiredCaps: [...ruleCaps(e.rule)],
      ...oneWay,
    });
  });

  const locsByRegion = new Map<RegionId, string[]>();
  for (const [loc, reg] of reach.graph.locations) {
    const a = locsByRegion.get(reg) ?? [];
    a.push(loc);
    locsByRegion.set(reg, a);
  }

  const areas: AreaDescriptor[] = [];
  for (const region of order) {
    const areaId = areaIdOf.get(region) as number;
    const locations = locsByRegion.get(region) ?? [];
    const placement = new Map<string, string>();
    for (const loc of locations) {
      const item = reach.placement.get(loc);
      if (item) placement.set(loc, item);
    }
    areas.push(
      composeArea({
        registry: ctx.registry,
        seed: reachSeed,
        areaId,
        regionId: region,
        role: reach.meta.roles.get(region) ?? "segment",
        depth,
        styleId,
        origin: [0, areaId * LANE, 0],
        portals: portalsByRegion.get(region) ?? [],
        locations,
        placement,
        itemCap,
      }),
    );
  }

  return {
    reach,
    descriptor: {
      areas,
      links,
      startAreaId: areaIdOf.get(reach.graph.start) as number,
      bounds: boxUnionAll(areas.map((a) => a.bounds)),
    },
  };
}
