# Authoring

For **content authors** — the people writing the mechanics, actors and puzzles a world is generated
from.

Content is authored **visually**, as schematics and graphs in the editor. There is no scripting
language: the node palette is generated from the API manifest, so a member that does not exist is a
node that is not in the list rather than a name that fails to compile.

| Page | |
|---|---|
| [`api-reference.md`](api-reference.md) | Every tier-1 class, struct and enum. ⚠ **Generated** from `manifest/tier1.toml` — see the note at the top of the file |
| `schematics.md` | _(planned: M18)_ the three tabs, inheritance, the `OVERRIDES` list |
| `graphs.md` | _(planned: M18)_ nodes, pins, wires, collections, loops |
| `spines-and-states.md` | _(planned: M19)_ macro-structure and world state |

## The one thing to know first

A **field** is a plain read and appears as a pure *get* node with no execution pins. A **method**
takes an argument, computes, or mutates, and appears as a *call* node with them. The shape of a node
tells you whether it costs anything before you read its name.
