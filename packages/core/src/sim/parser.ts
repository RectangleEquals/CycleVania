/**
 * Command parser — turns REPL text like `/use skyhook` or `goto 7` into a
 * `Command`. Throws on malformed input so the REPL can echo the error.
 */

import type { Command } from "./command.js";

export function parseCommand(input: string): Command {
  const s = input.trim().replace(/^\//, "");
  const parts = s.split(/\s+/);
  const verb = (parts[0] ?? "").toLowerCase();
  const arg = parts[1] ?? "";
  switch (verb) {
    case "goto":
    case "move": {
      const areaId = Number(arg);
      if (!Number.isFinite(areaId)) throw new Error(`goto: expected an area id, got "${arg}"`);
      return { k: "goto", areaId };
    }
    case "use":
      if (!arg) throw new Error("use: expected an item id");
      return { k: "use", itemId: arg };
    case "take":
      return { k: "take" };
    case "give":
      if (!arg) throw new Error("give: expected a capability");
      return { k: "give", cap: arg };
    case "why": {
      const areaId = Number(arg);
      if (!Number.isFinite(areaId)) throw new Error(`why: expected an area id, got "${arg}"`);
      return { k: "why", areaId };
    }
    case "reset":
      return { k: "reset" };
    case "solve":
      return { k: "solve" };
    default:
      throw new Error(`unknown command: "${verb}"`);
  }
}
