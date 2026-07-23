/**
 * `@cyclevania/examples` — illustrative, ready-to-run registry data + the three
 * shipped presets (crawler / classic / prime) that span the fidelity spectrum, plus
 * a Metroid-Prime-scale fixture that stress-tests the registry schema. None of this
 * is privileged: a host authors its own catalogs the same way. Import a preset,
 * `presetWorld(PRESET, seed)`, and generate.
 */

export { CAPABILITIES, GADGETS, GADGET_CATALOG } from "./gadget-catalog.js";
export { PUZZLES } from "./puzzle-catalog.js";
export { LINEAR, HUB_SPOKE, LOOP_HEAVY, EXAMPLE_TEMPLATE_POOL } from "./templates.js";
export { MODIFIERS, MODIFIER_POLICY } from "./modifiers.js";
export { CRAWLER, CLASSIC, PRIME, PRESETS, presetWorld, type Preset } from "./presets.js";
export { MP_REGISTRY_INPUT } from "./fixtures/mp.js";
