/**
 * The 3D view. Renders a composed Reach in world space (left-handed Z-up, mapped
 * to Three's Y-up): every non-air room cell as an instanced box coloured by
 * surface role, gadgets as gold markers, portals coloured by their gate, area
 * bounds as wire boxes, and inter-area links as lines. It never renders actual
 * kit geometry — it visualizes the abstract descriptor.
 */

import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { ReachResult, SimState, SimWorld, Vec3 } from "@cyclevania/core";
import { reachableAreaIds } from "@cyclevania/core";

export const ROLE_COLORS: Record<string, number> = {
  floor: 0x555a66,
  ceiling: 0x2a2f3a,
  wall: 0x3b6ea5,
  corner: 0x59c2c9,
  opening: 0xf4d35e,
};

const COLOR_GADGET = 0xffd24a;
const COLOR_PORTAL_OPEN = 0x5fd35f;
const COLOR_PORTAL_GATED = 0xd85a5a;
const COLOR_CURRENT = 0xffffff;
const COLOR_REACHABLE = 0x4fae64;
const COLOR_BLOCKED = 0xcaa24a;

// world (x,y,z, Z-up) → three (x, z, y, Y-up)
const v3 = (w: Vec3): THREE.Vector3 => new THREE.Vector3(w[0], w[2], w[1]);

export class InspectorScene {
  readonly renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera: THREE.PerspectiveCamera;
  private controls: OrbitControls;
  private content = new THREE.Group();
  private raycaster = new THREE.Raycaster();
  private pickBoxes: Array<{ mesh: THREE.Mesh; areaId: number }> = [];
  private areaWire = new Map<number, THREE.LineSegments>();
  private areaCenter = new Map<number, THREE.Vector3>();
  private player: THREE.Mesh;
  private areaClickCb: ((areaId: number) => void) | null = null;

  constructor(private container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.setSize(container.clientWidth, container.clientHeight);
    this.renderer.setClearColor(0x0c0e14, 1);
    container.appendChild(this.renderer.domElement);

    this.scene.background = new THREE.Color(0x0c0e14);
    this.scene.fog = new THREE.Fog(0x0c0e14, 200, 700);
    this.camera = new THREE.PerspectiveCamera(55, container.clientWidth / container.clientHeight, 0.5, 4000);
    this.camera.position.set(90, 120, 260);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;

    this.scene.add(new THREE.AmbientLight(0x8899bb, 1.1));
    const key = new THREE.DirectionalLight(0xffffff, 1.4);
    key.position.set(120, 260, 160);
    this.scene.add(key);
    this.scene.add(new THREE.GridHelper(1200, 48, 0x1a2030, 0x141826));
    this.scene.add(this.content);

    this.player = new THREE.Mesh(new THREE.SphereGeometry(2.2, 16, 12), new THREE.MeshBasicMaterial({ color: 0xffffff }));
    this.player.visible = false;
    this.scene.add(this.player);

    this.renderer.domElement.addEventListener("pointerdown", (e) => this.onPointer(e));
    window.addEventListener("resize", () => this.resize());
    this.animate();
  }

  onAreaClick(cb: (areaId: number) => void): void {
    this.areaClickCb = cb;
  }

  setReach(result: ReachResult): void {
    this.content.clear();
    this.pickBoxes = [];
    this.areaWire.clear();
    this.areaCenter.clear();

    const byRole = new Map<string, THREE.Vector3[]>();
    for (const area of result.descriptor.areas) {
      // wire box + pick proxy + center
      const size: Vec3 = [area.bounds.max[0] - area.bounds.min[0], area.bounds.max[1] - area.bounds.min[1], area.bounds.max[2] - area.bounds.min[2]];
      const center: Vec3 = [(area.bounds.min[0] + area.bounds.max[0]) / 2, (area.bounds.min[1] + area.bounds.max[1]) / 2, (area.bounds.min[2] + area.bounds.max[2]) / 2];
      const c3 = v3(center);
      this.areaCenter.set(area.areaId, c3);

      const boxGeo = new THREE.BoxGeometry(Math.max(1, size[0]) + 2, Math.max(1, size[2]) + 2, Math.max(1, size[1]) + 2);
      const wire = new THREE.LineSegments(new THREE.EdgesGeometry(boxGeo), new THREE.LineBasicMaterial({ color: COLOR_REACHABLE }));
      wire.position.copy(c3);
      this.content.add(wire);
      this.areaWire.set(area.areaId, wire);

      const pick = new THREE.Mesh(boxGeo, new THREE.MeshBasicMaterial({ visible: false }));
      pick.position.copy(c3);
      this.content.add(pick);
      this.pickBoxes.push({ mesh: pick, areaId: area.areaId });

      // cells
      for (const room of area.rooms) {
        const cs = (room.bounds.max[0] - room.bounds.min[0]) / Math.max(1, room.footprint[0]);
        for (const cell of room.cells) {
          if (cell.role === "air" || cell.kitId === null) continue;
          const wpos: Vec3 = [
            room.origin[0] + (cell.coord[0] + 0.5) * cs,
            room.origin[1] + (cell.coord[1] + 0.5) * cs,
            room.origin[2] + (cell.coord[2] + 0.5) * cs,
          ];
          const list = byRole.get(cell.role) ?? [];
          list.push(v3(wpos));
          byRole.set(cell.role, list);
        }
      }

      // gadgets
      for (const g of area.gadgets) {
        const m = new THREE.Mesh(new THREE.OctahedronGeometry(1.4), new THREE.MeshStandardMaterial({ color: COLOR_GADGET, emissive: 0x4a3a00 }));
        m.position.copy(v3(g.pos)).add(new THREE.Vector3(0, 2.5, 0));
        this.content.add(m);
      }

      // portals
      for (const p of area.portals) {
        const color = p.requires ? COLOR_PORTAL_GATED : COLOR_PORTAL_OPEN;
        const m = new THREE.Mesh(new THREE.ConeGeometry(1.3, 3, 8), new THREE.MeshStandardMaterial({ color }));
        m.position.copy(v3(p.spawn));
        this.content.add(m);
      }
    }

    // instanced cells per role
    const cellGeo = (size: number): THREE.BoxGeometry => new THREE.BoxGeometry(size, size, size);
    for (const [role, centers] of byRole) {
      const mat = new THREE.MeshStandardMaterial({ color: ROLE_COLORS[role] ?? 0x888888, roughness: 0.85, metalness: 0.05 });
      const inst = new THREE.InstancedMesh(cellGeo(1.7), mat, centers.length);
      const dummy = new THREE.Object3D();
      centers.forEach((c, i) => {
        dummy.position.copy(c);
        dummy.updateMatrix();
        inst.setMatrixAt(i, dummy.matrix);
      });
      inst.instanceMatrix.needsUpdate = true;
      this.content.add(inst);
    }

    // links between area centers
    const pts: THREE.Vector3[] = [];
    const colors: number[] = [];
    for (const link of result.descriptor.links) {
      const a = this.areaCenter.get(link.fromAreaId);
      const b = this.areaCenter.get(link.toAreaId);
      if (!a || !b) continue;
      const gated = (link.requiredCaps?.length ?? 0) > 0;
      const col = new THREE.Color(gated ? COLOR_PORTAL_GATED : COLOR_PORTAL_OPEN);
      pts.push(a.clone(), b.clone());
      colors.push(col.r, col.g, col.b, col.r, col.g, col.b);
    }
    if (pts.length) {
      const geo = new THREE.BufferGeometry().setFromPoints(pts);
      geo.setAttribute("color", new THREE.Float32BufferAttribute(colors, 3));
      this.content.add(new THREE.LineSegments(geo, new THREE.LineBasicMaterial({ vertexColors: true })));
    }

    this.frameCamera(result);
  }

  updateSim(world: SimWorld, state: SimState): void {
    const reach = reachableAreaIds(world, state.held);
    for (const [areaId, wire] of this.areaWire) {
      const mat = wire.material as THREE.LineBasicMaterial;
      mat.color.setHex(areaId === state.areaId ? COLOR_CURRENT : reach.has(areaId) ? COLOR_REACHABLE : COLOR_BLOCKED);
    }
    const c = this.areaCenter.get(state.areaId);
    if (c) {
      this.player.visible = true;
      this.player.position.copy(c).add(new THREE.Vector3(0, 14, 0));
    }
  }

  private frameCamera(result: ReachResult): void {
    const b = result.descriptor.bounds;
    const center = v3([(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2]);
    const span = Math.max(b.max[0] - b.min[0], b.max[1] - b.min[1], 60);
    this.controls.target.copy(center);
    this.camera.position.set(center.x + span * 0.5, center.y + span * 0.7, center.z + span * 1.1);
    this.controls.update();
  }

  private onPointer(e: PointerEvent): void {
    if (!this.areaClickCb) return;
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    const hits = this.raycaster.intersectObjects(this.pickBoxes.map((p) => p.mesh), false);
    const first = hits[0];
    if (first) {
      const found = this.pickBoxes.find((p) => p.mesh === first.object);
      if (found) this.areaClickCb(found.areaId);
    }
  }

  private resize(): void {
    const w = this.container.clientWidth;
    const h = this.container.clientHeight;
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
  }

  private animate = (): void => {
    requestAnimationFrame(this.animate);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };
}
