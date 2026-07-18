import type { ConnectorKind, Traversal } from "../types.js";

/**
 * Connector archetype — how two sockets get joined (curved hall, 45° tunnel, open
 * seam, vertical shaft, snake). Connectors are composed as thin corridor-rooms.
 */
export interface ConnectorArchetype {
  id: string;
  kind: ConnectorKind;
  traversal: Traversal;
  tags?: string[];
}
