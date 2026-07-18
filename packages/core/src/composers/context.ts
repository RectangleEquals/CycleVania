import type { Registry } from "../registries/registry.js";

/** Everything a composer needs: the game's data registry + a root seed. */
export interface ComposeContext {
  registry: Registry;
  seed: string | number;
}
