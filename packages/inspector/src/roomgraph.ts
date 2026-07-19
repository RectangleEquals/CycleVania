/**
 * Room-level graph for Play mode. The core simulator moves between AREAS; Play
 * mode wants to walk ROOM-to-ROOM, so we derive a finer graph from the composed
 * descriptors: intra-area connectors (ungated) + inter-area portals (gated by the
 * region edge's rule). Exits carry the socket world-position so the view can put
 * an interactable marker there, plus a diegetic label. Gate checks reuse the
 * core's `evalRule` so play logic never diverges from the solver.
 */

import { boxContainsPoint, type ReachResult, type Registry, type Rule, type Vec3 } from "@cyclevania/core";

export interface RoomRef {
  ri: number;
  areaId: number;
  nodeId: string;
}
export const roomKey = (r: RoomRef): string => `${r.ri}:${r.areaId}:${r.nodeId}`;

export interface RoomExit {
  socketId: string;
  to: RoomRef;
  gate?: Rule; // undefined = open
  kind: "connector" | "portal";
  pos: Vec3; // socket world position (marker + focus)
  label: string;
}

export interface RoomGadget {
  itemId: string;
  cap?: string;
  locationId: string;
  pos: Vec3;
}

export interface RoomNode {
  ref: RoomRef;
  role: string;
  kind: string;
  exits: RoomExit[];
  gadgets: RoomGadget[];
  bounds: { min: Vec3; max: Vec3 };
}

export class RoomGraph {
  readonly nodes = new Map<string, RoomNode>();

  constructor(reaches: ReachResult[], _registry: Registry) {
    reaches.forEach((reach, ri) => {
      // index sockets → room, per area
      const socketRoom = new Map<string, { nodeId: string; pos: Vec3 }>(); // `${areaId}|${socketId}`
      for (const area of reach.descriptor.areas) {
        for (const room of area.rooms) {
          this.nodes.set(roomKey({ ri, areaId: area.areaId, nodeId: room.nodeId }), {
            ref: { ri, areaId: area.areaId, nodeId: room.nodeId },
            role: room.role,
            kind: room.kind,
            exits: [],
            gadgets: [],
            bounds: room.bounds,
          });
          for (const s of room.sockets) socketRoom.set(`${area.areaId}|${s.id}`, { nodeId: room.nodeId, pos: s.pos });
        }
        // assign gadgets to the room whose bounds contain them
        for (const g of area.gadgets) {
          const host = area.rooms.find((r) => boxContainsPoint(r.bounds, g.pos)) ?? area.rooms[0];
          if (host) this.nodes.get(roomKey({ ri, areaId: area.areaId, nodeId: host.nodeId }))?.gadgets.push({ itemId: g.itemId, ...(g.cap ? { cap: g.cap } : {}), locationId: g.locationId, pos: g.pos });
        }
      }

      // intra-area connectors (both directions, ungated)
      for (const area of reach.descriptor.areas) {
        for (const c of area.connectors) {
          const a = socketRoom.get(`${area.areaId}|${c.fromSocket}`);
          const b = socketRoom.get(`${area.areaId}|${c.toSocket}`);
          if (!a || !b) continue;
          this.addExit({ ri, areaId: area.areaId, nodeId: a.nodeId }, { socketId: c.fromSocket, to: { ri, areaId: area.areaId, nodeId: b.nodeId }, kind: "connector", pos: a.pos, label: `passage to ${b.nodeId}` });
          this.addExit({ ri, areaId: area.areaId, nodeId: b.nodeId }, { socketId: c.toSocket, to: { ri, areaId: area.areaId, nodeId: a.nodeId }, kind: "connector", pos: b.pos, label: `passage to ${a.nodeId}` });
        }
      }

      // inter-area portals: match the two portals sharing an edge key
      const byKey = new Map<string, Array<{ areaId: number; regionId: string; nodeId: string; pos: Vec3; requires?: Rule; oneWay?: boolean; edge: { from: string; to: string } }>>();
      for (const area of reach.descriptor.areas) {
        for (const p of area.portals) {
          if (!p.edge) continue;
          const sr = socketRoom.get(`${area.areaId}|portal:${p.key}`);
          if (!sr) continue;
          const list = byKey.get(p.key) ?? [];
          list.push({ areaId: area.areaId, regionId: area.regionId, nodeId: sr.nodeId, pos: sr.pos, ...(p.requires ? { requires: p.requires } : {}), ...(p.oneWay ? { oneWay: true } : {}), edge: p.edge });
          byKey.set(p.key, list);
        }
      }
      for (const [, ends] of byKey) {
        if (ends.length < 2) continue;
        const [x, y] = ends;
        if (!x || !y) continue;
        const fromSide = x.regionId === x.edge.from ? x : y;
        const toSide = fromSide === x ? y : x;
        // forward: gated by requires
        this.addExit({ ri, areaId: fromSide.areaId, nodeId: fromSide.nodeId }, { socketId: `portal`, to: { ri, areaId: toSide.areaId, nodeId: toSide.nodeId }, kind: "portal", pos: fromSide.pos, label: `gateway to area ${toSide.areaId}`, ...(fromSide.requires ? { gate: fromSide.requires } : {}) });
        // reverse: open unless one-way
        if (!fromSide.oneWay) {
          this.addExit({ ri, areaId: toSide.areaId, nodeId: toSide.nodeId }, { socketId: `portal`, to: { ri, areaId: fromSide.areaId, nodeId: fromSide.nodeId }, kind: "portal", pos: toSide.pos, label: `gateway to area ${fromSide.areaId}` });
        }
      }
    });
  }

  private addExit(from: RoomRef, exit: RoomExit): void {
    this.nodes.get(roomKey(from))?.exits.push(exit);
  }

  /** The first room of the reach's start area. */
  startRoom(reach: ReachResult): RoomRef | null {
    const area = reach.descriptor.areas.find((a) => a.areaId === reach.descriptor.startAreaId) ?? reach.descriptor.areas[0];
    const room = area?.rooms[0];
    return area && room ? { ri: reachIndexOf(reach), areaId: area.areaId, nodeId: room.nodeId } : null;
  }
}

// ReachResult doesn't carry its own index; the graph keys by the array position,
// so callers pass reachIndex explicitly. This helper is only for startRoom when
// the caller already knows the reach — resolved via a WeakMap set at build time.
const reachIdx = new WeakMap<ReachResult, number>();
export function tagReachIndex(reach: ReachResult, i: number): void {
  reachIdx.set(reach, i);
}
function reachIndexOf(reach: ReachResult): number {
  return reachIdx.get(reach) ?? 0;
}
