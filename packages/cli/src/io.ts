/**
 * Buffered IO so every subcommand is testable in-process (no shell, no real
 * stdio). `runCli` returns the accumulated `stdout`/`stderr` + exit `code`; the
 * `main.ts` bin flushes them to the real streams.
 */

import type { DiagLevel, DiagnosticsConfig, DiagnosticsSink } from "@cyclevania/core";

export class CliIO {
  private outBuf = "";
  private errBuf = "";

  out(s: string): void {
    this.outBuf += s;
  }
  outln(s = ""): void {
    this.outBuf += s + "\n";
  }
  err(s: string): void {
    this.errBuf += s;
  }
  errln(s = ""): void {
    this.errBuf += s + "\n";
  }
  get stdout(): string {
    return this.outBuf;
  }
  get stderr(): string {
    return this.errBuf;
  }
}

/** Build a DiagnosticsConfig that routes events to stderr as stable, grep-able lines. */
export function stderrDiagnostics(io: CliIO, level: DiagLevel): DiagnosticsConfig {
  const sink: DiagnosticsSink = {
    emit(e) {
      const loc = e.path ? ` ${e.path}` : "";
      io.errln(`[${e.level}] ${e.code}${loc} — ${e.message}`);
    },
  };
  return { level, sink };
}

export const LOG_LEVELS: DiagLevel[] = ["error", "warn", "info", "debug", "trace"];
