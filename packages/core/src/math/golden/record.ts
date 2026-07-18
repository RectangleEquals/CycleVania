/**
 * Shared record-scripts for golden-vector parity. Both the dev generator
 * (`scripts/gen-golden.ts`) and the hermetic `golden.test.ts` run these against
 * the ported math, so the recorded shape can never drift between them.
 */

export interface RngModuleLike {
  Rng: new (seed: number | string) => {
    next(): number;
    int(min: number, max: number): number;
    range(min: number, max: number): number;
    chance(p: number): boolean;
    pick<T>(arr: readonly T[]): T;
    fork(label: string): { next(): number };
  };
  fnv1a: (s: string, basis?: number) => number;
}

export interface TrigModuleLike {
  dsin: (a: number) => number;
  dcos: (a: number) => number;
  datan: (x: number) => number;
  datan2: (y: number, x: number) => number;
  yawFromDirection: (dx: number, dy: number) => number;
}

export function recordRng(mod: RngModuleLike): unknown {
  const seeds: Array<string | number> = ["alpha", "bravo", "charlie", 1, 42, 999999];
  const out: Record<string, unknown> = {};
  for (const seed of seeds) {
    const rng = new mod.Rng(seed);
    const nexts: number[] = [];
    for (let i = 0; i < 16; i++) nexts.push(rng.next());
    const ints: number[] = [];
    for (let i = 0; i < 8; i++) ints.push(rng.int(0, 1000));
    const ranges: number[] = [];
    for (let i = 0; i < 8; i++) ranges.push(rng.range(-10, 10));
    const chances: boolean[] = [];
    for (let i = 0; i < 16; i++) chances.push(rng.chance(0.5));
    const arr = ["a", "b", "c", "d", "e"];
    const picks: string[] = [];
    for (let i = 0; i < 8; i++) picks.push(rng.pick(arr));
    const child = rng.fork("child");
    const forkNexts: number[] = [];
    for (let i = 0; i < 8; i++) forkNexts.push(child.next());
    out[String(seed)] = { nexts, ints, ranges, chances, picks, forkNexts };
  }
  out["__fnv"] = {
    empty: mod.fnv1a(""),
    hello: mod.fnv1a("hello"),
    long: mod.fnv1a("the quick brown fox"),
    chained: mod.fnv1a("child", 0x1234),
  };
  return out;
}

export function recordTrig(mod: TrigModuleLike): unknown {
  const angles = [0, 0.5, 1, Math.PI / 2, Math.PI, -Math.PI / 2, -Math.PI, 2, 3.5, -3.5, 6.28318, 100, -100, 0.0001];
  const atanInputs = [-5, -1, -0.5, 0, 0.5, 1, 5, 100];
  const atan2Pairs: Array<[number, number]> = [
    [1, 1], [1, -1], [-1, 1], [-1, -1], [0, 1], [0, -1], [1, 0], [-1, 0], [0, 0], [3, 4],
  ];
  const yawDirs: Array<[number, number]> = [[1, 0], [0, 1], [-1, 0], [0, -1], [1, 1]];
  return {
    angles,
    sin: angles.map((a) => mod.dsin(a)),
    cos: angles.map((a) => mod.dcos(a)),
    atanInputs,
    atan: atanInputs.map((x) => mod.datan(x)),
    atan2Pairs,
    atan2: atan2Pairs.map(([y, x]) => mod.datan2(y, x)),
    yawDirs,
    yaw: yawDirs.map(([dx, dy]) => mod.yawFromDirection(dx, dy)),
  };
}
