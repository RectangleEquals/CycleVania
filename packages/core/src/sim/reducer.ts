/**
 * The reducer — `step(world, state, cmd)` applies one command, returning a fresh
 * state + a diegetic-ready message. Pure: the input state is never mutated. Uses
 * the SAME rule logic (`evalRule`/`missingCaps`) as the solver — one logic
 * implementation everywhere.
 */

import { evalRule, missingCaps } from "../logic/index.js";
import type { RegionId } from "../graph/index.js";
import { autosolve } from "./autosolve.js";
import type { Command } from "./command.js";
import { cloneState, initSim, type SimState } from "./state.js";
import { neighbors, type SimWorld } from "./world.js";

export interface SimResult {
  state: SimState;
  ok: boolean;
  message: string;
}

const done = (s: SimState, ok: boolean, message: string): SimResult => {
  s.log.push((ok ? "· " : "✗ ") + message);
  return { state: s, ok, message };
};

/** Clear volatile flags set in a region we're leaving. */
function clearVolatileOnLeave(s: SimState, leaving: RegionId): void {
  for (const [flag, region] of [...s.volatileFlags]) {
    if (region === leaving) {
      s.held.removeFlag(flag);
      s.volatileFlags.delete(flag);
    }
  }
}

/** BFS a path of region ids from `from` to `to` over currently-open neighbors. */
function pathTo(world: SimWorld, s: SimState, to: RegionId): RegionId[] | undefined {
  const prev = new Map<RegionId, RegionId>();
  const seen = new Set<RegionId>([s.at]);
  const queue: RegionId[] = [s.at];
  while (queue.length > 0) {
    const cur = queue.shift() as RegionId;
    if (cur === to) {
      const path: RegionId[] = [];
      let node: RegionId | undefined = to;
      while (node !== undefined && node !== s.at) {
        path.unshift(node);
        node = prev.get(node);
      }
      return path;
    }
    for (const n of neighbors(world, cur, s.held)) {
      if (n.ok && !seen.has(n.to)) {
        seen.add(n.to);
        prev.set(n.to, cur);
        queue.push(n.to);
      }
    }
  }
  return undefined;
}

export function step(world: SimWorld, state: SimState, cmd: Command): SimResult {
  if (cmd.k === "reset") return { state: initSim(world), ok: true, message: "reset" };
  if (cmd.k === "solve") {
    const r = autosolve(world);
    return { state: r.state, ok: r.success, message: `autosolved → ${r.state.at}` };
  }

  const s = cloneState(state);
  switch (cmd.k) {
    case "move": {
      const n = neighbors(world, s.at, s.held).find((x) => x.to === cmd.to);
      if (!n) return done(s, false, `${cmd.to} is not adjacent to ${s.at}`);
      if (!n.ok) return done(s, false, `blocked into ${cmd.to} — need: ${missingCaps(n.link.rule, s.held).join(", ") || "an event"}`);
      clearVolatileOnLeave(s, s.at);
      s.at = cmd.to;
      s.visited.add(cmd.to);
      if (n.link.oneWay) s.openedOneWays.add(`${n.link.from}->${n.link.to}`);
      return done(s, true, `moved to ${cmd.to}`);
    }
    case "goto": {
      const path = pathTo(world, s, cmd.to);
      if (!path) return done(s, false, `no open path to ${cmd.to}`);
      for (const hop of path) {
        clearVolatileOnLeave(s, s.at);
        s.at = hop;
        s.visited.add(hop);
      }
      return done(s, true, `went to ${cmd.to}`);
    }
    case "take": {
      const node = world.nodes.get(s.at);
      const took: string[] = [];
      for (const loc of node?.locations ?? []) {
        if (s.collected.has(loc.id)) continue;
        s.collected.add(loc.id);
        if (loc.itemId !== undefined) {
          s.inventory.add(loc.itemId);
          const item = world.items.get(loc.itemId);
          for (const c of item?.grants ?? []) s.held.add(c);
          took.push(loc.itemId);
        }
        for (const f of world.flagSetters.get(loc.id) ?? []) s.held.addFlag(f);
      }
      if (took.length === 0) return done(s, false, "nothing to take here");
      return done(s, true, `took ${took.join(", ")}`);
    }
    case "use": {
      if (!s.inventory.has(cmd.itemId)) return done(s, false, `you don't hold "${cmd.itemId}"`);
      const item = world.items.get(cmd.itemId);
      for (const c of item?.grants ?? []) s.held.add(c);
      return done(s, true, `used ${cmd.itemId}`);
    }
    case "interact": {
      const p = world.puzzles.find((x) => x.instanceId === cmd.puzzleId);
      if (!p) return done(s, false, `no such interactable "${cmd.puzzleId}"`);
      if (!evalRule(p.condition, s.held)) return done(s, false, `cannot solve ${cmd.puzzleId} yet — need: ${missingCaps(p.condition, s.held).join(", ") || "an event"}`);
      s.solvedPuzzles.add(p.instanceId);
      s.held.addFlag(p.instanceId);
      if (p.outcome.kind === "grant-capability") s.held.add(p.outcome.capability);
      return done(s, true, `solved ${cmd.puzzleId}`);
    }
    case "give": {
      s.held.add(cmd.cap);
      return done(s, true, `granted "${cmd.cap}"`);
    }
    case "see": {
      const opts = neighbors(world, s.at, s.held);
      const open = opts.filter((o) => o.ok).map((o) => o.to);
      const blocked = opts.filter((o) => !o.ok).map((o) => `${o.to} (need ${missingCaps(o.link.rule, s.held).join(",") || "event"})`);
      const here = (world.nodes.get(s.at)?.locations ?? []).filter((l) => !s.collected.has(l.id)).length;
      return done(s, true, `at ${s.at}: ${here} item(s) here; open→ ${open.join(",") || "—"}; blocked→ ${blocked.join("; ") || "—"}`);
    }
    case "why": {
      const n = neighbors(world, s.at, s.held).find((x) => x.to === cmd.to);
      if (!n) return done(s, true, `${cmd.to} is not adjacent`);
      if (n.ok) return done(s, true, `${cmd.to} is open`);
      return done(s, true, `${cmd.to} needs: ${missingCaps(n.link.rule, s.held).join(", ") || "an event"}`);
    }
  }
}
