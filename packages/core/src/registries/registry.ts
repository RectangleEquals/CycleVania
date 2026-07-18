/**
 * The data-injection surface. `defineRegistry` takes a game's data (grid,
 * GeometryKit, item catalog, lock vocabulary, room/connector/style registries,
 * complexity curve), validates it, and normalizes it into a `Registry` the
 * composers consume. Locks are resolved to concrete rules at build time.
 */

import type { Capability } from "../logic/index.js";
import type { ProgressionItem } from "../graph/region-graph.js";
import type { GridConfig } from "./grid-config.js";
import type { GeometryKit } from "./geometry-kit.js";
import type { ItemCatalogInput, ItemDef, ItemClass } from "./item-catalog.js";
import { RULE_DSL, type ChallengeTemplate, type LockVocabularyInput, type ResolvedLock } from "./lock-vocabulary.js";
import type { RoomArchetype } from "./room-archetypes.js";
import type { ConnectorArchetype } from "./connector-archetypes.js";
import type { StyleDef } from "./style-registry.js";
import { DEFAULT_COMPLEXITY, type ComplexityConfig } from "./complexity-config.js";

export interface RegistryInput {
  grid: GridConfig;
  geometryKit: GeometryKit;
  items: ItemCatalogInput;
  locks: LockVocabularyInput;
  rooms?: Record<string, RoomArchetype>;
  connectors?: Record<string, ConnectorArchetype>;
  styles?: Record<string, StyleDef>;
  complexity?: ComplexityConfig;
}

export interface Registry {
  grid: GridConfig;
  geometryKit: GeometryKit;
  items: {
    defs: Map<string, ItemDef>;
    startCaps: Capability[];
    /** {id, grants} for the solver — the progression subset. */
    progression: ProgressionItem[];
    byClass: (c: ItemClass) => ItemDef[];
    /** capability → the lock name whose recipe realizes it (for the composer). */
    profileFor: (cap: Capability) => ItemDef | undefined;
  };
  locks: Map<string, ResolvedLock>;
  rooms: Map<string, RoomArchetype>;
  connectors: Map<string, ConnectorArchetype>;
  styles: Map<string, StyleDef>;
  complexity: ComplexityConfig;
}

const toMap = <T>(o?: Record<string, T>): Map<string, T> => new Map(Object.entries(o ?? {}));

export function defineRegistry(input: RegistryInput): Registry {
  // --- items ---
  const defs = new Map<string, ItemDef>();
  const grantIndex = new Map<Capability, ItemDef>();
  for (const d of input.items.catalog) {
    if (defs.has(d.id)) throw new Error(`defineRegistry: duplicate item id "${d.id}"`);
    if (d.class === "progression" && !d.grants) {
      throw new Error(`defineRegistry: progression item "${d.id}" must declare "grants"`);
    }
    defs.set(d.id, d);
    if (d.grants && !grantIndex.has(d.grants)) grantIndex.set(d.grants, d);
  }
  const progression: ProgressionItem[] = input.items.catalog
    .filter((d) => d.class === "progression")
    .map((d) => ({ id: d.id, grants: d.grants as Capability }));

  // --- locks: normalize builder|template → resolved rule + recipe ---
  const locks = new Map<string, ResolvedLock>();
  for (const [name, def] of Object.entries(input.locks)) {
    const tmpl: ChallengeTemplate = typeof def === "function" ? { solvedBy: def, recipe: "gate" } : def;
    locks.set(name, { name, rule: tmpl.solvedBy(RULE_DSL), recipe: tmpl.recipe, def: tmpl });
  }

  return {
    grid: input.grid,
    geometryKit: input.geometryKit,
    items: {
      defs,
      startCaps: input.items.startCaps ?? [],
      progression,
      byClass: (c) => [...defs.values()].filter((d) => d.class === c),
      profileFor: (cap) => grantIndex.get(cap),
    },
    locks,
    rooms: toMap(input.rooms),
    connectors: toMap(input.connectors),
    styles: toMap(input.styles),
    complexity: input.complexity ?? DEFAULT_COMPLEXITY,
  };
}
