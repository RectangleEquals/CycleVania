import { GenError } from "@cyclevania/core";
import type { DiagnosticsConfig } from "@cyclevania/core";
import { parseArgs } from "../args.js";
import type { CliIO } from "../io.js";
import { resolveSource } from "../sources.js";
import { makeRegistry } from "../pipeline.js";

/** validate <source> — defineRegistry (+ template static checks); exit 1 on GenError. */
export async function run(rest: string[], io: CliIO, diag: DiagnosticsConfig): Promise<number> {
  const p = parseArgs(rest);
  const source = p._[0];
  if (!source) {
    io.errln("validate: missing <source>");
    return 1;
  }
  let src;
  try {
    src = await resolveSource(source);
  } catch (e) {
    io.errln(`validate: ${(e as Error).message}`);
    return 1;
  }
  if (src.kind === "descriptor") {
    io.outln("valid — world descriptor file (no dataset to validate)");
    return 0;
  }
  try {
    const reg = makeRegistry(src, diag);
    io.outln(`valid — fingerprint ${reg.fingerprint} · ${reg.capabilities.length} capabilities · ${reg.gadgets.length} gadgets · ${reg.puzzles.length} puzzles`);
    return 0;
  } catch (e) {
    if (e instanceof GenError) {
      io.errln(`invalid: [${e.code}] ${e.message}`);
      return 1;
    }
    io.errln(`invalid: ${(e as Error).message}`);
    return 1;
  }
}
