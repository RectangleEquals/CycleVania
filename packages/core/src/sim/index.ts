export { buildSimWorld, neighbors, reachableAreaIds } from "./world.js";
export type { SimWorld, SimArea, SimLink, SimGadget, SimItemInfo } from "./world.js";
export { initSim, cloneState } from "./state.js";
export type { SimState } from "./state.js";
export type { Command } from "./command.js";
export { parseCommand } from "./parser.js";
export { step } from "./reducer.js";
export type { SimResult } from "./reducer.js";
export { autosolve } from "./autosolve.js";
