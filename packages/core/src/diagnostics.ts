/**
 * The one structured, write-only diagnostics channel, threaded through every
 * generation layer. Two audiences: a host integrating CycleVania (actionable
 * warnings) and a CycleVania developer (watching the generator think at
 * `debug`/`trace`).
 *
 * Load-bearing rules (see redesign 12):
 *  1. Diagnostics NEVER affect generation — output is byte-identical under any
 *     sink and any level. Nothing in generation may read a sink or branch on the
 *     configured level beyond the emitter's own filtering.
 *  2. Codes are stable API (grep-able). Relaxations, GenErrors, and validation
 *     issues all emit with the same code strings they surface elsewhere.
 *  3. Configurable everywhere (registry default, per-run override, CLI flag,
 *     Inspector panel). Core itself never writes to a console.
 *  4. A throwing sink is caught and disabled for the run — it never propagates
 *     into the pipeline.
 */

export type DiagLevel = "error" | "warn" | "info" | "debug" | "trace";

export interface DiagEvent {
  level: DiagLevel;
  code: string;
  message: string;
  path?: string;
  details?: Record<string, unknown>;
}

export interface DiagnosticsSink {
  emit(e: DiagEvent): void;
}

export interface DiagnosticsConfig {
  level: DiagLevel;
  sink: DiagnosticsSink;
}

/** The default sink: discards everything. */
export const SILENT_SINK: DiagnosticsSink = { emit(): void {} };

/** A collecting sink for tests and tooling. */
export class MemorySink implements DiagnosticsSink {
  readonly events: DiagEvent[] = [];
  emit(e: DiagEvent): void {
    this.events.push(e);
  }
}

const ORDER: Record<DiagLevel, number> = { error: 0, warn: 1, info: 2, debug: 3, trace: 4 };

/** The default config a caller gets when none is supplied. */
export const DEFAULT_DIAGNOSTICS: DiagnosticsConfig = { level: "warn", sink: SILENT_SINK };

interface SinkState {
  broken: boolean;
}

/**
 * The emitter handed down through generation. `child(segment)` appends to the
 * path prefix so every event is located (`reach3/area:r2/space:s4`). Children
 * share the parent's broken-sink guard, so one throwing sink disables the whole
 * run's emission, once.
 */
export class Diag {
  private readonly cfg: DiagnosticsConfig;
  private readonly prefix: string;
  private readonly state: SinkState;

  constructor(cfg: DiagnosticsConfig = DEFAULT_DIAGNOSTICS, prefix = "", state?: SinkState) {
    this.cfg = cfg;
    this.prefix = prefix;
    this.state = state ?? { broken: false };
  }

  private emit(level: DiagLevel, code: string, message: string, path?: string, details?: Record<string, unknown>): void {
    if (ORDER[level] > ORDER[this.cfg.level]) return;
    if (this.state.broken) return;
    const full = this.prefix ? (path ? `${this.prefix}/${path}` : this.prefix) : path;
    const e: DiagEvent = { level, code, message };
    if (full !== undefined) e.path = full;
    if (details !== undefined) e.details = details;
    try {
      this.cfg.sink.emit(e);
    } catch {
      this.state.broken = true; // a throwing sink is disabled for the remainder of the run
    }
  }

  error(code: string, message: string, path?: string, details?: Record<string, unknown>): void {
    this.emit("error", code, message, path, details);
  }
  warn(code: string, message: string, path?: string, details?: Record<string, unknown>): void {
    this.emit("warn", code, message, path, details);
  }
  info(code: string, message: string, path?: string, details?: Record<string, unknown>): void {
    this.emit("info", code, message, path, details);
  }
  debug(code: string, message: string, path?: string, details?: Record<string, unknown>): void {
    this.emit("debug", code, message, path, details);
  }
  trace(code: string, message: string, path?: string, details?: Record<string, unknown>): void {
    this.emit("trace", code, message, path, details);
  }

  /** A child emitter whose events are prefixed with `segment` under this path. */
  child(segment: string): Diag {
    const p = this.prefix ? `${this.prefix}/${segment}` : segment;
    return new Diag(this.cfg, p, this.state);
  }
}
