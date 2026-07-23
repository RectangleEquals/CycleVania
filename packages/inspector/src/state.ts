/**
 * The single inspector store. All view/selection/mode state lives here (never on
 * the descriptors, which stay immutable). Views subscribe and re-render on change.
 */

import type { WorldComposer, ReachResult, WorldDescriptor, DiagEvent, GenProgress } from "@cyclevania/core";

export type ViewId = "world" | "mission" | "schedule" | "skeleton" | "volume" | "geometry" | "play";

export const PHASE_1_VIEWS: ViewId[] = ["world", "mission", "schedule"];

export interface Settings {
  presetName: string;
  seed: string;
  reaches: number;
  geometry: boolean;
}

export type Selection =
  | { kind: "none" }
  | { kind: "reach"; reachIndex: number }
  | { kind: "region"; reachIndex: number; regionId: string }
  | { kind: "location"; reachIndex: number; locationId: string };

export interface State {
  settings: Settings;
  world: WorldComposer | undefined;
  reaches: ReachResult[];
  descriptor: WorldDescriptor | undefined;
  view: ViewId;
  selection: Selection;
  diagnostics: DiagEvent[];
  diagLevel: "error" | "warn" | "info" | "debug" | "trace";
  diagCollapsed: boolean;
  progress: GenProgress | undefined;
  generating: boolean;
  sphereStep: number; // -1 = show all
}

type Listener = () => void;

class Store {
  readonly state: State = {
    settings: { presetName: "classic", seed: "cyclevania-demo", reaches: 1, geometry: true },
    world: undefined,
    reaches: [],
    descriptor: undefined,
    view: "world",
    selection: { kind: "none" },
    diagnostics: [],
    diagLevel: "warn",
    diagCollapsed: false,
    progress: undefined,
    generating: false,
    sphereStep: -1,
  };

  private readonly listeners = new Set<Listener>();

  subscribe(l: Listener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }

  emit(): void {
    for (const l of this.listeners) l();
  }

  set(patch: Partial<State>): void {
    Object.assign(this.state, patch);
    this.emit();
  }

  select(sel: Selection): void {
    this.state.selection = sel;
    this.emit();
  }
}

export const store = new Store();

/** Is `sel` the currently-selected region? (one shared selection model across views) */
export function isRegionSelected(reachIndex: number, regionId: string): boolean {
  const s = store.state.selection;
  return s.kind === "region" && s.reachIndex === reachIndex && s.regionId === regionId;
}
