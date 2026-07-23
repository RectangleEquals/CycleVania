/**
 * Cross-cutting shared type aliases used across layers. Kept tiny and dependency-
 * free so any module can import them without a cycle.
 */

/** How a connection/socket is crossed. `drop` implies one-way unless re-opened. */
export type Traversal = "walk" | "climb" | "crawl" | "drop" | "swim" | "vertical";

/** Space kinds decided at L2, realized at L3. */
export type SpaceKind = "room" | "outdoor" | "connector";

/** Local surface classification from an outward normal (L3/L4 share this). */
export type SurfaceKind = "floor" | "wall" | "ceiling" | "slope" | "overhang";
