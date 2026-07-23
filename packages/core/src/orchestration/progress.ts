/**
 * Structured progress — enough for a real loading screen. Overall `fraction` is
 * weighted by a static per-phase table (finish dominates) so the bar moves
 * honestly rather than sprinting through cheap phases and stalling at meshing.
 */

import type { GenPhase } from "../world/index.js";

export interface GenProgress {
  phase: GenPhase;
  label: string;
  areaIndex?: number;
  areasTotal?: number;
  fraction: number; // 0..1 overall, monotonic non-decreasing
  phaseFraction: number; // 0..1 within the current phase
  elapsedMs: number;
}

export const PHASE_WEIGHTS: Record<GenPhase, number> = {
  template: 2,
  selection: 2,
  graph: 2,
  fill: 2,
  skeleton: 8,
  volume: 14,
  finish: 60,
  assemble: 4,
};

const TOTAL_WEIGHT = Object.values(PHASE_WEIGHTS).reduce((s, w) => s + w, 0);

const LABELS: Record<GenPhase, string> = {
  template: "Resolving template",
  selection: "Selecting content",
  graph: "Building mission graph",
  fill: "Placing items",
  skeleton: "Laying out space",
  volume: "Composing volume",
  finish: "Meshing geometry",
  assemble: "Assembling",
};

export class ProgressTracker {
  private readonly completed = new Set<GenPhase>();
  private cumulative = 0;

  /** Mark a phase complete and produce the progress report for it. */
  complete(phase: GenPhase, elapsedMs: number): GenProgress {
    if (!this.completed.has(phase)) {
      this.completed.add(phase);
      this.cumulative += PHASE_WEIGHTS[phase];
    }
    return {
      phase,
      label: LABELS[phase],
      fraction: this.cumulative / TOTAL_WEIGHT,
      phaseFraction: 1,
      elapsedMs,
    };
  }
}
