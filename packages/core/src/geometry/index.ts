export { quantizeNormal, isQuantized } from "./fidelity.js";
export { dualContour } from "./mesher.js";
export type { Mesh } from "./mesher.js";
export { meshToKit } from "./kit.js";
export type { GeneratedKit, GeneratedPiece, PieceInstance, PieceMeta, SurfaceKind, KitOptions } from "./kit.js";
export { occupancyGrid, cellOf, isSolidAt, collideSphere } from "./collision.js";
export type { OccupancyGrid } from "./collision.js";
export { dressArea } from "./dress.js";
export type { DressingAnchor } from "./dress.js";
