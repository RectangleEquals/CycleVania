/**
 * The 3D view. Renders ONLY the active scope (World shows areas; Reach shows one
 * reach's areas; Area shows one area's rooms + connections; Room shows one room's
 * cells + exits + contents) so you can never click into an outer/adjacent scope.
 * Air cells are non-pickable (rays pass through). Supports a selection highlight,
 * an x-ray cutaway (near-camera / in-cone structural cells fade and stop blocking
 * picks; floors never fade), and a constrained Play camera.
 */

import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { AreaDescriptor, CellDescriptor, ReachResult, RoomDescriptor, SimState, SimWorld, Vec3, WorldBox } from "@cyclevania/core";
import { reachableAreaIds } from "@cyclevania/core";

export const ROLE_COLORS: Record<string, number> = { floor: 0x555a66, ceiling: 0x2a2f3a, wall: 0x3b6ea5, corner: 0x59c2c9, opening: 0xf4d35e };
const C_GADGET = 0xffd24a;
const C_OPEN = 0x5fd35f;
const C_GATED = 0xd85a5a;
const C_CURRENT = 0xffffff;
const C_REACH = 0x4fae64;
const C_BLOCK = 0xcaa24a;
const C_AREA = 0x37507a;
const C_ROOM = 0x2c3550;
const C_SELECT = 0xffe08a;

export const CONTENT_COLORS: Record<string, number> = { gadget: 0xffd24a, cache: 0xd9b45a, prop: 0x8a9bb0, hazard: 0xe07a3a, enemy: 0xd85a7a, switch: 0x59c2c9, boss: 0xff5a5a, puzzle: 0x9a7ad8 };
const contentColor = (k: string): number => CONTENT_COLORS[k] ?? 0xffd24a;

export type Scope =
  | { level: "world" }
  | { level: "reach"; ri: number }
  | { level: "area"; ri: number; areaId: number }
  | { level: "room"; ri: number; areaId: number; nodeId: string };

export interface PickResult {
  kind: "area" | "room" | "cell" | "gadget" | "connection";
  ri: number;
  areaId?: number;
  nodeId?: string;
  cell?: CellDescriptor;
  itemId?: string;
  socketId?: string;
  gated?: boolean;
  pos?: Vec3;
  box?: WorldBox;
}

export interface XrayConfig {
  on: boolean;
  dist: number;
  coneDeg: number;
  mask: Record<string, boolean>; // role → xray-affected
}

const v3 = (w: Vec3): THREE.Vector3 => new THREE.Vector3(w[0], w[2], w[1]);
const centerOf = (b: WorldBox): THREE.Vector3 => v3([(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2]);
const spanOf = (b: WorldBox): number => Math.max(b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2], 4);

export class InspectorScene {
  readonly renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera: THREE.PerspectiveCamera;
  private controls: OrbitControls;
  private group = new THREE.Group();
  private selGroup = new THREE.Group();
  private raycaster = new THREE.Raycaster();
  private pickables: THREE.Object3D[] = [];
  private cellMeshes: THREE.Mesh[] = [];
  private areaProxy = new Map<number, THREE.LineSegments>();
  private reaches: ReachResult[] = [];
  private scope: Scope = { level: "world" };
  private xray: XrayConfig = { on: true, dist: 34, coneDeg: 55, mask: { ceiling: true, wall: true, corner: true, opening: false } };
  private play = false;
  private fwd = new THREE.Vector3();

  constructor(private container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.setSize(container.clientWidth, container.clientHeight);
    container.appendChild(this.renderer.domElement);
    this.scene.background = new THREE.Color(0x0c0e14);
    this.camera = new THREE.PerspectiveCamera(55, container.clientWidth / container.clientHeight, 0.4, 16000);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.scene.add(new THREE.AmbientLight(0x8899bb, 1.2));
    const key = new THREE.DirectionalLight(0xffffff, 1.35);
    key.position.set(120, 320, 160);
    this.scene.add(key, new THREE.GridHelper(6000, 100, 0x151b28, 0x10141d), this.group, this.selGroup);
    window.addEventListener("resize", () => this.resize());
    this.animate();
  }

  setData(reaches: ReachResult[]): void {
    this.reaches = reaches;
  }

  setXray(cfg: Partial<XrayConfig>): void {
    this.xray = { ...this.xray, ...cfg, mask: { ...this.xray.mask, ...(cfg.mask ?? {}) } };
  }

  getScope(): Scope {
    return this.scope;
  }

  setScope(scope: Scope): void {
    this.scope = scope;
    this.selGroup.clear();
    this.render();
    this.frameScope();
  }

  // ---- rendering ----
  private render(): void {
    this.group.clear();
    this.pickables = [];
    this.cellMeshes = [];
    this.areaProxy.clear();
    const s = this.scope;
    if (s.level === "world") this.reaches.forEach((r, ri) => this.renderReach(ri, r, false));
    else if (s.level === "reach") { const r = this.reaches[s.ri]; if (r) this.renderReach(s.ri, r, true); }
    else if (s.level === "area") { const a = this.area(s.ri, s.areaId); if (a) this.renderArea(s.ri, a); }
    else { const room = this.room(s.ri, s.areaId, s.nodeId); if (room) this.renderRoom(s.ri, s.areaId, room); }
  }

  private renderReach(ri: number, reach: ReachResult, withConnectors: boolean): void {
    const portalPos = new Map<string, THREE.Vector3[]>();
    for (const area of reach.descriptor.areas) {
      this.proxy(area.bounds, C_AREA, { kind: "area", ri, areaId: area.areaId, box: area.bounds }, area.areaId);
      for (const p of area.portals) {
        const at = v3(p.spawn);
        if (p.edge) { const l = portalPos.get(p.key) ?? []; l.push(at); portalPos.set(p.key, l); }
      }
      if (withConnectors) {
        for (const room of area.rooms) this.instanceCellsAt(room.cells, room.origin, this.roomCs(room), 1.5);
        for (const c of area.connectors) if (c.origin && c.cellSize) this.instanceCellsAt(c.cells, c.origin, c.cellSize, 1.4);
      }
    }
    this.drawLinks(reach, portalPos);
  }

  private renderArea(ri: number, area: AreaDescriptor): void {
    for (const room of area.rooms) {
      this.proxy(room.bounds, C_ROOM, { kind: "room", ri, areaId: area.areaId, nodeId: room.nodeId, box: room.bounds });
      this.instanceCellsAt(room.cells, room.origin, this.roomCs(room), 1.6);
    }
    // connector corridors (real geometry)
    for (const c of area.connectors) if (c.origin && c.cellSize) this.instanceCellsAt(c.cells, c.origin, c.cellSize, 1.4);
    // populated content (props/hazards/enemies) as non-pickable decor so rooms read as inhabited
    for (const room of area.rooms) {
      const rcs = this.roomCs(room);
      for (const cell of room.cells) for (const c of cell.contents) {
        if (c.kind === "gadget") continue; // gadgets drawn as pickable markers below
        const at = this.cellWorld(room.origin, cell.coord, rcs).add(new THREE.Vector3(0, rcs * 0.6, 0));
        const m = new THREE.Mesh(new THREE.OctahedronGeometry(1.1), new THREE.MeshStandardMaterial({ color: contentColor(c.kind), emissive: 0x0c0c0c }));
        m.position.copy(at);
        this.group.add(m);
      }
    }
    // gadgets + portals (pickable)
    for (const g of area.gadgets) this.marker(v3(g.pos).add(new THREE.Vector3(0, 2.4, 0)), C_GADGET, "gadget", { kind: "gadget", ri, areaId: area.areaId, itemId: g.itemId, pos: g.pos });
    for (const p of area.portals) this.marker(v3(p.spawn), p.requires ? C_GATED : C_OPEN, "cone", { kind: "connection", ri, areaId: area.areaId, socketId: `portal:${p.key}`, gated: !!p.requires, pos: p.spawn });
  }

  private renderRoom(ri: number, areaId: number, room: RoomDescriptor): void {
    for (const cell of room.cells) {
      const isAir = cell.role === "air" || cell.kitId === null;
      const mesh = new THREE.Mesh(
        new THREE.BoxGeometry(1.85, 1.85, 1.85),
        isAir
          ? new THREE.MeshBasicMaterial({ color: 0x263048, wireframe: true, transparent: true, opacity: 0.18 })
          : new THREE.MeshStandardMaterial({ color: ROLE_COLORS[cell.role] ?? 0x888888, roughness: 0.82, transparent: true, opacity: 1 }),
      );
      mesh.position.copy(this.cellPos(room, cell));
      mesh.userData = { role: cell.role, air: isAir, pick: { kind: "cell", ri, areaId, nodeId: room.nodeId, cell, box: room.bounds } satisfies PickResult };
      this.group.add(mesh);
      if (!isAir) {
        this.cellMeshes.push(mesh);
        this.pickables.push(mesh);
      }
    }
    // exits (sockets) + contents markers
    for (const sk of room.sockets) this.marker(v3(sk.pos), sk.gate ? C_GATED : C_OPEN, "cone", { kind: "connection", ri, areaId, nodeId: room.nodeId, socketId: sk.id, gated: !!sk.gate, pos: sk.pos });
    for (const cell of room.cells) for (const c of cell.contents) {
      const p = this.cellPos(room, cell).add(new THREE.Vector3(0, 1.6, 0));
      this.marker(p, contentColor(c.kind), "gadget", { kind: "gadget", ri, areaId, nodeId: room.nodeId, ...(c.ref ? { itemId: c.ref } : {}), pos: [p.x, p.z, p.y] });
    }
  }

  // ---- helpers ----
  private drawLinks(reach: ReachResult, portalPos: Map<string, THREE.Vector3[]>): void {
    const pts: THREE.Vector3[] = [];
    const cols: number[] = [];
    for (const link of reach.descriptor.links) {
      const from = reach.descriptor.areas.find((a) => a.areaId === link.fromAreaId);
      const to = reach.descriptor.areas.find((a) => a.areaId === link.toAreaId);
      if (!from || !to) continue;
      const key = from.portals.find((p) => p.edge && p.edge.from === from.regionId && p.edge.to === to.regionId)?.key;
      const ps = key ? portalPos.get(key) : undefined;
      if (ps && ps.length >= 2) {
        const c = new THREE.Color((link.requiredCaps?.length ?? 0) > 0 ? C_GATED : C_OPEN);
        pts.push(ps[0]!.clone(), ps[1]!.clone());
        cols.push(c.r, c.g, c.b, c.r, c.g, c.b);
      }
    }
    if (pts.length) {
      const g = new THREE.BufferGeometry().setFromPoints(pts);
      g.setAttribute("color", new THREE.Float32BufferAttribute(cols, 3));
      this.group.add(new THREE.LineSegments(g, new THREE.LineBasicMaterial({ vertexColors: true })));
    }
  }

  private roomCs(room: RoomDescriptor): number {
    return (room.bounds.max[0] - room.bounds.min[0]) / Math.max(1, room.footprint[0]);
  }

  private cellWorld(origin: Vec3, coord: readonly [number, number, number], cs: number): THREE.Vector3 {
    return v3([origin[0] + (coord[0] + 0.5) * cs, origin[1] + (coord[1] + 0.5) * cs, origin[2] + (coord[2] + 0.5) * cs]);
  }

  private instanceCellsAt(cells: readonly CellDescriptor[], origin: Vec3, cs: number, size: number): void {
    const byRole = new Map<string, THREE.Vector3[]>();
    for (const cell of cells) {
      if (cell.role === "air" || cell.kitId === null) continue;
      const l = byRole.get(cell.role) ?? [];
      l.push(this.cellWorld(origin, cell.coord, cs));
      byRole.set(cell.role, l);
    }
    for (const [role, centers] of byRole) {
      const inst = new THREE.InstancedMesh(new THREE.BoxGeometry(size, size, size), new THREE.MeshStandardMaterial({ color: ROLE_COLORS[role] ?? 0x888888, roughness: 0.85 }), centers.length);
      const d = new THREE.Object3D();
      centers.forEach((c, i) => { d.position.copy(c); d.updateMatrix(); inst.setMatrixAt(i, d.matrix); });
      inst.instanceMatrix.needsUpdate = true;
      this.group.add(inst);
    }
  }

  private proxy(box: WorldBox, color: number, pick: PickResult, areaId?: number): void {
    const size: Vec3 = [box.max[0] - box.min[0], box.max[1] - box.min[1], box.max[2] - box.min[2]];
    const geo = new THREE.BoxGeometry(Math.max(1, size[0]), Math.max(1, size[2]), Math.max(1, size[1]));
    const wire = new THREE.LineSegments(new THREE.EdgesGeometry(geo), new THREE.LineBasicMaterial({ color }));
    wire.position.copy(centerOf(box));
    this.group.add(wire);
    if (areaId !== undefined) this.areaProxy.set(areaId, wire);
    const hit = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ visible: false }));
    hit.position.copy(centerOf(box));
    hit.userData = { pick };
    this.group.add(hit);
    this.pickables.push(hit);
  }

  private marker(at: THREE.Vector3, color: number, shape: "cone" | "gadget", pick: PickResult): void {
    const geo = shape === "cone" ? new THREE.ConeGeometry(1.5, 3.4, 8) : new THREE.OctahedronGeometry(1.6);
    const m = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({ color, emissive: shape === "gadget" ? 0x4a3a00 : 0x0 }));
    m.position.copy(at);
    m.userData = { pick, marker: true };
    this.group.add(m);
    this.pickables.push(m);
  }

  private cellPos(room: RoomDescriptor, cell: CellDescriptor): THREE.Vector3 {
    const cs = (room.bounds.max[0] - room.bounds.min[0]) / Math.max(1, room.footprint[0]);
    return v3([room.origin[0] + (cell.coord[0] + 0.5) * cs, room.origin[1] + (cell.coord[1] + 0.5) * cs, room.origin[2] + (cell.coord[2] + 0.5) * cs]);
  }

  // ---- selection ----
  setSelected(pick: PickResult | null): void {
    this.selGroup.clear();
    if (!pick) return;
    const wire = (w: number, h: number, d: number, at: THREE.Vector3): void => {
      const ls = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(w, h, d)), new THREE.LineBasicMaterial({ color: C_SELECT }));
      ls.position.copy(at);
      this.selGroup.add(ls);
    };
    if (pick.kind === "cell" && pick.cell && pick.nodeId) {
      const room = this.room(pick.ri, pick.areaId!, pick.nodeId);
      if (room) wire(2.3, 2.3, 2.3, this.cellPos(room, pick.cell));
    } else if (pick.box) {
      const sz: Vec3 = [pick.box.max[0] - pick.box.min[0], pick.box.max[1] - pick.box.min[1], pick.box.max[2] - pick.box.min[2]];
      wire(Math.max(1, sz[0]) + 1.5, Math.max(1, sz[2]) + 1.5, Math.max(1, sz[1]) + 1.5, centerOf(pick.box));
    } else if (pick.pos) {
      const ring = new THREE.Mesh(new THREE.TorusGeometry(2.8, 0.4, 8, 22), new THREE.MeshBasicMaterial({ color: C_SELECT }));
      ring.position.copy(v3(pick.pos));
      this.selGroup.add(ring);
    }
  }

  // ---- camera ----
  focus(pick: PickResult): void {
    if (pick.box && (pick.kind === "area" || pick.kind === "room")) this.focusBox(pick.box);
    else if (pick.kind === "cell" && pick.cell && pick.nodeId) { const r = this.room(pick.ri, pick.areaId!, pick.nodeId); if (r) this.focusPoint(this.cellPos(r, pick.cell), spanOf(r.bounds) * 0.4); }
    else if (pick.pos) this.focusPoint(v3(pick.pos), 22);
  }

  focusBox(b: WorldBox): void {
    this.focusPoint(centerOf(b), spanOf(b));
  }

  focusPoint(center: THREE.Vector3, span: number): void {
    this.controls.target.copy(center);
    this.camera.position.copy(center).add(new THREE.Vector3(span * 0.4, span * 0.7, span * 1.1));
    this.controls.update();
  }

  private frameScope(): void {
    const s = this.scope;
    if (s.level === "world") { const b = this.reaches.map((r) => r.descriptor.bounds).reduce(union, this.reaches[0]?.descriptor.bounds ?? unit()); this.focusBox(b); }
    else if (s.level === "reach") { const r = this.reaches[s.ri]; if (r) this.focusBox(r.descriptor.bounds); }
    else if (s.level === "area") { const a = this.area(s.ri, s.areaId); if (a) this.focusBox(a.bounds); }
    else { const r = this.room(s.ri, s.areaId, s.nodeId); if (r) this.focusBox(r.bounds); }
  }

  setPlayCamera(on: boolean, box?: WorldBox): void {
    this.play = on;
    if (on && box) {
      this.controls.enablePan = false;
      this.controls.maxDistance = spanOf(box) * 1.4;
      this.controls.minDistance = 3;
      this.focusPoint(centerOf(box), spanOf(box) * 0.7);
    } else {
      this.controls.enablePan = true;
      this.controls.maxDistance = Infinity;
      this.controls.minDistance = 0;
    }
  }

  highlightSim(ri: number, world: SimWorld, state: SimState): void {
    if (this.scope.level !== "world" && this.scope.level !== "reach") return;
    const reachable = reachableAreaIds(world, state.held);
    const reach = this.reaches[ri];
    if (!reach) return;
    for (const area of reach.descriptor.areas) {
      const proxy = this.areaProxy.get(area.areaId);
      if (proxy) (proxy.material as THREE.LineBasicMaterial).color.setHex(area.areaId === state.areaId ? C_CURRENT : reachable.has(area.areaId) ? C_REACH : C_BLOCK);
    }
  }

  /** Nearest specific object (cell/gadget/connection) wins; else nearest container proxy. */
  pick(clientX: number, clientY: number): PickResult | null {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((clientX - rect.left) / rect.width) * 2 - 1, -((clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    const hits = this.raycaster.intersectObjects(this.pickables, false);
    let proxy: PickResult | null = null;
    for (const h of hits) {
      if ((h.object.userData as { noPick?: boolean }).noPick) continue;
      const pick = (h.object.userData as { pick?: PickResult }).pick;
      if (!pick) continue;
      if (pick.kind === "cell" || pick.kind === "gadget" || pick.kind === "connection") return pick;
      if (!proxy) proxy = pick;
    }
    return proxy;
  }

  private applyXray(): void {
    const active = this.xray.on && this.scope.level === "room";
    this.camera.getWorldDirection(this.fwd);
    const coneCos = Math.cos((this.xray.coneDeg * Math.PI) / 180);
    const camPos = this.camera.position;
    for (const mesh of this.cellMeshes) {
      const role = (mesh.userData as { role: string }).role;
      const mat = mesh.material as THREE.MeshStandardMaterial;
      let hidden = false;
      if (active && role !== "floor" && this.xray.mask[role]) {
        const to = mesh.position.clone().sub(camPos);
        const dist = to.length();
        if (dist < this.xray.dist && to.normalize().dot(this.fwd) > coneCos) hidden = true;
      }
      mat.opacity = hidden ? 0.06 : 1;
      mat.depthWrite = !hidden;
      (mesh.userData as { noPick?: boolean }).noPick = hidden;
    }
  }

  private area(ri: number, areaId: number): AreaDescriptor | undefined {
    return this.reaches[ri]?.descriptor.areas.find((a) => a.areaId === areaId);
  }
  private room(ri: number, areaId: number, nodeId: string): RoomDescriptor | undefined {
    return this.area(ri, areaId)?.rooms.find((r) => r.nodeId === nodeId);
  }

  private resize(): void {
    this.camera.aspect = this.container.clientWidth / this.container.clientHeight;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(this.container.clientWidth, this.container.clientHeight);
  }

  private animate = (): void => {
    requestAnimationFrame(this.animate);
    this.controls.update();
    this.applyXray();
    this.renderer.render(this.scene, this.camera);
  };
}

function union(a: WorldBox, b: WorldBox): WorldBox {
  return { min: [Math.min(a.min[0], b.min[0]), Math.min(a.min[1], b.min[1]), Math.min(a.min[2], b.min[2])], max: [Math.max(a.max[0], b.max[0]), Math.max(a.max[1], b.max[1]), Math.max(a.max[2], b.max[2])] };
}
function unit(): WorldBox {
  return { min: [0, 0, 0], max: [1, 1, 1] };
}
