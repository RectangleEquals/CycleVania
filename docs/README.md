# CycleVania documentation

> **Status: pre-alpha.** Most pages listed here are not written yet — entries marked _(planned: Mxx)_
> name the milestone that writes them. The *structure* is committed now so that docs land in the right
> place as they are written, instead of accreting into a flat pile that has to be untangled later.

## Start here — what are you trying to do?

| I want to… | Go to |
|---|---|
| Understand what CycleVania is, and generate a first world | `getting-started.md` _(planned: M26)_ |
| Understand **how generation works**, conceptually | [`concepts/`](concepts/) |
| **Author content** — mechanics, actors, puzzles — visually | [`authoring/`](authoring/) |
| **Plug generation into my own game** | [`hosting/`](hosting/) |
| Use the **editor** | [`editor/`](editor/) |
| Work on **CycleVania itself** | [`contributing/`](contributing/) |

Most people need exactly one of these. If you are building a game, you will live in `hosting/` and
`authoring/`, and dip into `concepts/` when something surprises you.

## The sections

| Section | Audience | Contains |
|---|---|---|
| **`concepts/`** | everyone | The mental model: determinism, the L0–L6 pipeline, lazy generation, the linearity dials. Read once; refer back. No API detail. |
| **`authoring/`** | content authors | The visual authoring surface: schematics, graphs, spines, state graphs, and the generated [API reference](authoring/api-reference.md). |
| **`hosting/`** | integrators | Getting generation into a real game: the TypeScript package, Rust/WASM, project layout, consuming the output, shipping a build. |
| **`editor/`** | anyone tuning a world | The dev tool: panels, dials, the seed lab, the generation trace. |
| **`contributing/`** | people working on the SDK | Build workflow, the determinism rules the engine enforces on itself, crate architecture. |

Anything genuinely **forefront** — a getting-started page, a changelog, an FAQ — sits at the top level
rather than being buried a folder deep.

## How these docs are organised

Five rules, in priority order. They exist so the docs stay findable as they grow.

1. **Audience-first, not subsystem-first.** Folders map to *what a reader is doing*, not to how the
   engine is built internally. Someone asking "how do I make my world less linear?" should not have to
   know that is an L2 concern — that page lives in `concepts/`, and the dials it describes are
   cross-linked from `authoring/` and `editor/`.
2. **One canonical home per topic.** Every subject is explained *once*, in the section that owns it.
   Everywhere else **links** to it. Duplicated explanation is how documentation rots: two copies drift,
   and neither is trustworthy.
3. **Every folder has a `README.md` that routes within it.** Combined with this page, that keeps any
   topic at most **two clicks** from the front door — the "simple path front-and-center, depth one click
   deeper" rule the editor follows too.
4. **Concepts link down; how-to links up.** A concept page ends by pointing at the pages that *use* it;
   a how-to page opens by pointing at the concept it assumes. A reader can enter from either direction.
5. **`lowercase-kebab-case`** for every file and folder, matching the source tree and avoiding
   case-sensitivity differences between Windows and CI.

### Where does a new page go?

Ask what the reader was doing when they needed it:

- *"Why does the generator behave this way?"* → `concepts/`
- *"How do I author this?"* → `authoring/`
- *"How do I call this from my game?"* → `hosting/` (per-language pages under it)
- *"Which button does what?"* → `editor/`
- *"How do I build/test the engine?"* → `contributing/`

If a page genuinely serves two audiences, it still gets **one** home — the one whose reader needs it
most — plus a link from the other. If you cannot decide, it is usually a sign the page is really two
pages.

> **Note:** the private design notes under `.notes/Design/` and `.notes/Implementation/` are *not*
> these docs. Those are working design records for the maintainers and are gitignored; these are the
> public, reader-facing manual. Design decisions get *summarised* here, not copy-pasted.
