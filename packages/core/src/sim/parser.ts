/** Parse a slash command string into a `Command` (the REPL front-end). */

import type { Command } from "./command.js";

export function parseCommand(input: string): Command | undefined {
  const parts = input.trim().replace(/^\//, "").split(/\s+/);
  const verb = parts[0]?.toLowerCase();
  const arg = parts[1];
  switch (verb) {
    case "move":
      return arg !== undefined ? { k: "move", to: arg } : undefined;
    case "goto":
      return arg !== undefined ? { k: "goto", to: arg } : undefined;
    case "take":
      return { k: "take" };
    case "use":
      return arg !== undefined ? { k: "use", itemId: arg } : undefined;
    case "interact":
      return arg !== undefined ? { k: "interact", puzzleId: arg } : undefined;
    case "see":
      return { k: "see" };
    case "why":
      return arg !== undefined ? { k: "why", to: arg } : undefined;
    case "give":
      return arg !== undefined ? { k: "give", cap: arg } : undefined;
    case "reset":
      return { k: "reset" };
    case "solve":
      return { k: "solve" };
    default:
      return undefined;
  }
}
