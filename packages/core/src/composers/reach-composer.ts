/**
 * ReachComposer — holds all Areas of a Reach. It runs the grammar (region graph +
 * solvable placement), then lays the Areas out DIEGETICALLY in 3D world space: a
 * branching tree that fans across XY, with vault branches dipping in −Z and the
 * capstone/terminal pushed further out. A Reach should read like a 3D world map —
 * direction and depth you can hold in your head — not a straight line. Each
 * region's interior is delegated to the AreaComposer.
 */

import { dcos, dsin, datan2 } from "../math/trig.js";
import { boxUnionAll } from "../math/geom.js";
import type { Vec3 } from "../math/vec.js";
import { ruleCaps, type Capability } from "../logic/index.js";
import { generateReach, type GeneratedReach } from "../template/grammar.js";
import type { ReachTemplate } from "../template/template.js";
import type { ProgressionItem, RegionId } from "../graph/region-graph.js";
import type { AreaDescriptor, AreaLink, ReachDescriptor } from "../descriptors/descriptor.js";
import { composeArea, type PortalRequest } from "./area-composer.js";
import type { ComposeContext } from "./context.js";

export interface ComposeReachOptions {
  template: ReachTemplate;
  reachIndex?: number;
  depth?: number;
  startCaps?: Iterable<Capability>;
  styleId?: string;
  /** World offset for this whole Reach (so a World of Reaches doesn't overlap). */
  origin?: Vec3;
  /** Progression items to place THIS reach (default: the registry's full set). Keep this small — a
   *  reach should introduce a handful of gadgets, not the whole catalog, or the hub floods. */
  gadgets?: readonly ProgressionItem[];
}

export interface ReachResult {
  reach: GeneratedReach;
  descriptor: ReachDescriptor;
}

const STEP = 150; // world distance between connected area centres
const SPREAD = 0.85; // radians between sibling branches
const VAULT_DROP = 70; // −Z dip for vault branches

export function composeReach(ctx: ComposeContext, opts: ComposeReachOptions): ReachResult {
  const reachIndex = opts.reachIndex ?? 0;
  const depth = opts.depth ?? reachIndex * 8;
  const styleId = opts.styleId ?? [...ctx.registry.styles.keys()][0] ?? "default";
  const reachSeed = `${ctx.seed}:reach${reachIndex}`;
  const base: Vec3 = opts.origin ?? [0, 0, 0];

  const reach = generateReach({
    seed: reachSeed,
    template: opts.template,
    items: opts.gadgets ?? ctx.registry.items.progression,
    ...(opts.startCaps ? { startCaps: opts.startCaps } : {}),
  });

  // forward BFS from the start region → order + parent map
  const order: RegionId[] = [reach.graph.start];
  const seen = new Set<RegionId>([reach.graph.start]);
  const parent = new Map<RegionId, RegionId>();
  for (let h = 0; h < order.length; h++) {
    const u = order[h] as RegionId;
    for (const e of reach.graph.edges) {
      if (e.from === u && !seen.has(e.to)) {
        seen.add(e.to);
        parent.set(e.to, u);
        order.push(e.to);
      }
    }
  }
  for (const r of reach.graph.regions) if (!seen.has(r)) order.push(r);
  const areaIdOf = new Map<RegionId, number>();
  order.forEach((r, i) => areaIdOf.set(r, i + 1));

  const pushInto = <K, V>(m: Map<K, V[]>, k: K, v: V): void => {
    const list = m.get(k) ?? [];
    list.push(v);
    m.set(k, list);
  };

  // children (in BFS order) for the fan-out layout
  const children = new Map<RegionId, RegionId[]>();
  for (const r of order) {
    const par = parent.get(r);
    if (par) pushInto(children, par, r);
  }

  // diegetic 3D positions: fan children around each parent's outward heading
  const pos = new Map<RegionId, Vec3>();
  const heading = new Map<RegionId, number>(); // XY angle (radians)
  pos.set(reach.graph.start, base);
  heading.set(reach.graph.start, Math.PI / 2); // +Y
  for (const u of order) {
    const kids = children.get(u) ?? [];
    const baseAng = heading.get(u) ?? Math.PI / 2;
    const p0 = pos.get(u) as Vec3;
    kids.forEach((c, k) => {
      const ang = baseAng + (k - (kids.length - 1) / 2) * SPREAD;
      const role = reach.meta.roles.get(c);
      const step = role === "capstone" ? STEP * 1.25 : role === "vault" ? STEP * 0.7 : STEP;
      const z = role === "vault" ? -VAULT_DROP : role === "capstone" ? 30 : 0;
      pos.set(c, [p0[0] + dcos(ang) * step, p0[1] + dsin(ang) * step, p0[2] + z]);
      heading.set(c, role === "vault" ? datan2(dsin(ang), dcos(ang)) : ang);
    });
  }

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
    links.push({ fromAreaId: areaIdOf.get(e.from) as number, toAreaId: areaIdOf.get(e.to) as number, requires: e.rule, requiredCaps: [...ruleCaps(e.rule)], ...oneWay });
  });

  const locsByRegion = new Map<RegionId, string[]>();
  for (const [loc, reg] of reach.graph.locations) pushInto(locsByRegion, reg, loc);

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
        origin: pos.get(region) ?? base,
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
