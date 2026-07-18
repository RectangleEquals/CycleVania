/**
 * Shared primitive type aliases used across composers, registries and spatial.
 * Kept dependency-free so any module can import them without cycles.
 */

/** How a socket/edge is traversed — walk today, the rest as the game supports them. */
export type Traversal = "walk" | "climb" | "crawl" | "drop" | "open" | "ladder" | "rope" | "vertical";

/** Geometry snap policy: PS2-era angle palette by default, or arbitrary. */
export type SnapPolicy = "ps2" | "free";

/** How a connector between two sockets is shaped. */
export type ConnectorKind = "straight" | "curved" | "ramp45" | "open" | "vertical" | "snake";

/** A face (or the interior) of a subdivided grid cell. */
export type CellFace = "px" | "nx" | "py" | "ny" | "pz" | "nz" | "interior";

/** The kind of aperture a socket is (not always a door). */
export type SocketKind =
  | "arch"
  | "cave-mouth"
  | "open"
  | "threshold"
  | "crawlspace"
  | "climb-face"
  | "drop-hatch"
  | "vent";

/** What kind of space a room realizes. */
export type RoomKind =
  | "indoor"
  | "outdoor"
  | "arena"
  | "vault"
  | "boss-chamber"
  | "hub"
  | "junction"
  | "shrine"
  | "secret";
