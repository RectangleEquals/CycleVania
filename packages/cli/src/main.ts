/**
 * `@cyclevania/cli` entry — headless generate / validate / soak / report / diff /
 * export-diagram over the public core API. `runCli(argv)` runs a single command
 * fully in-process, returning buffered stdout/stderr + an exit code (tests never
 * spawn shells); the bin block at the bottom flushes to the real streams.
 *
 * Invoke the bin with native type stripping:
 *   node --experimental-strip-types packages/cli/src/main.ts <command> …
 */

import { fileURLToPath } from "node:url";
import type { DiagLevel } from "@cyclevania/core";
import { parseArgs, strFlag } from "./args.js";
import { CliIO, stderrDiagnostics, LOG_LEVELS } from "./io.js";
import { run as generate } from "./commands/generate.js";
import { run as validate } from "./commands/validate.js";
import { run as soak } from "./commands/soak.js";
import { run as report } from "./commands/report.js";
import { run as diff } from "./commands/diff.js";
import { run as exportDiagram } from "./commands/export-diagram.js";

const COMMANDS: Record<string, (rest: string[], io: CliIO, diag: ReturnType<typeof stderrDiagnostics>) => Promise<number>> = {
  generate,
  validate,
  soak,
  report,
  diff,
  "export-diagram": exportDiagram,
};

function printHelp(io: CliIO): void {
  io.outln("cyclevania — deterministic procgen CLI");
  io.outln("");
  io.outln("Usage: cyclevania <command> <source> [options]");
  io.outln("");
  io.outln("Commands:");
  io.outln("  generate <source> [--seed S] [--reaches N] [--geometry] [-o out.json] [--save-bundle b.json]");
  io.outln("  validate <source>");
  io.outln("  soak <source> --seeds N [--autosolve] [--reaches R]");
  io.outln("  report <source> --seed S -o report.md [--geometry]");
  io.outln("  diff <sourceA> <sourceB> [--seed S] [--seedA X] [--seedB Y]");
  io.outln("  export-diagram <source> --seed S --view mission [-o out.mmd]");
  io.outln("");
  io.outln("<source>: a preset (classic/crawler/prime/mp), a module#export, a bundle JSON, or a descriptor JSON.");
  io.outln("Global: --log-level <error|warn|info|debug|trace> (default warn)");
}

export async function runCli(argv: readonly string[]): Promise<{ code: number; stdout: string; stderr: string }> {
  const io = new CliIO();
  const sub = argv[0];
  const rest = argv.slice(1);

  if (sub === undefined || sub === "--help" || sub === "-h" || sub === "help") {
    printHelp(io);
    return { code: sub === undefined ? 1 : 0, stdout: io.stdout, stderr: io.stderr };
  }

  const globals = parseArgs(rest);
  const lvl = strFlag(globals, "log-level") as DiagLevel | undefined;
  const level: DiagLevel = lvl && LOG_LEVELS.includes(lvl) ? lvl : "warn";
  const diag = stderrDiagnostics(io, level);

  const cmd = COMMANDS[sub];
  if (!cmd) {
    io.errln(`unknown command "${sub}"`);
    printHelp(io);
    return { code: 1, stdout: io.stdout, stderr: io.stderr };
  }

  let code = 0;
  try {
    code = await cmd(rest, io, diag);
  } catch (e) {
    io.errln(`error: ${(e as Error).message}`);
    code = 1;
  }
  return { code, stdout: io.stdout, stderr: io.stderr };
}

// --- bin entry (only when executed directly, not when imported by tests) ---
const invokedDirectly = process.argv[1] !== undefined && fileURLToPath(import.meta.url) === process.argv[1];
if (invokedDirectly) {
  void runCli(process.argv.slice(2)).then((r) => {
    if (r.stdout) process.stdout.write(r.stdout);
    if (r.stderr) process.stderr.write(r.stderr);
    process.exit(r.code);
  });
}
