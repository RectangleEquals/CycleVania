/**
 * Tiny hand-rolled arg parser (no commander): positionals in `_`, everything else
 * in `flags`. `--k v`, `--k=v`, `-o v`, and bare boolean `--flag` are supported;
 * flags named in `booleanFlags` never consume the next token.
 */

export interface ParsedArgs {
  _: string[];
  flags: Record<string, string | boolean>;
}

export function parseArgs(argv: readonly string[], booleanFlags: readonly string[] = []): ParsedArgs {
  const _: string[] = [];
  const flags: Record<string, string | boolean> = {};
  const boolSet = new Set(booleanFlags);
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i] as string;
    if (a === "-o") {
      flags["out"] = argv[i + 1] ?? "";
      i++;
      continue;
    }
    if (a.startsWith("--")) {
      const body = a.slice(2);
      const eq = body.indexOf("=");
      if (eq >= 0) {
        flags[body.slice(0, eq)] = body.slice(eq + 1);
        continue;
      }
      if (boolSet.has(body)) {
        flags[body] = true;
        continue;
      }
      const next = argv[i + 1];
      if (next !== undefined && !next.startsWith("--") && next !== "-o") {
        flags[body] = next;
        i++;
      } else {
        flags[body] = true;
      }
    } else {
      _.push(a);
    }
  }
  return { _, flags };
}

export const strFlag = (p: ParsedArgs, name: string): string | undefined => (typeof p.flags[name] === "string" ? (p.flags[name] as string) : undefined);
export const boolFlag = (p: ParsedArgs, name: string): boolean => p.flags[name] === true || p.flags[name] === "true";
export const intFlag = (p: ParsedArgs, name: string, dflt: number): number => {
  const v = strFlag(p, name);
  if (v === undefined) return dflt;
  const n = Number.parseInt(v, 10);
  return Number.isFinite(n) ? n : dflt;
};
