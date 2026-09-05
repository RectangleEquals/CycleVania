/**
 * **The Content Browser** — Unreal's, and there is exactly one of it.
 *
 * ⚠ **A first build had a `Content Drawer` button in the status bar *and* a separate `Content` dock**:
 * two names, two surfaces, one job. ▶ **The drawer is the browser.** Docking it into the layout changes
 * where it sits and nothing else, and `⊕ Add` on the toolbar opens the same create menu the drawer's
 * right-click opens — a convenience for when the drawer is closed, never a parallel route.
 *
 * `10-editor.md` §2, §9b.
 */

import { icon, iconForPath } from "./icons.ts";

/** Every extension the content root may hold, and what to call it. */
export const CONTENT_KINDS: { ext: string; label: string }[] = [
  { ext: "cvs", label: "Schematic" },
  { ext: "cvspine", label: "Spine" },
  { ext: "cvstate", label: "State" },
  { ext: "cvcurve", label: "Curve" },
  { ext: "cvunlock", label: "Unlocks" },
  { ext: "cvtags", label: "Tags" },
];

/**
 * ⚠ **The content root holds two populations** — `10-editor.md` §2. Authored `.cv*` sources open an
 * editor; imported assets have none, and that is not a gap: a mesh is authored in a DCC tool.
 */
export const IMPORTED_EXTS = ["glb", "gltf", "obj"];

export interface ContentFilter {
  /** ⚠ **Several at once**, each toggled alone — Unreal's filters stack. */
  kinds: string[];
  folder: string;
  search: string;
}

export interface Entry {
  path: string;
  name: string;
  kind: string;
  /** ⚠ An imported asset has no editor; double-click reveals it instead of opening nothing. */
  imported: boolean;
}

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

const kindOfExt = (ext: string) =>
  CONTENT_KINDS.find((k) => k.ext === ext)?.label ?? (IMPORTED_EXTS.includes(ext) ? "Mesh" : ext);

/** Split content paths into the folders under the current one, and the files in it. */
export function contentAt(files: string[], filter: ContentFilter): {
  folders: string[];
  files: Entry[];
} {
  const prefix = filter.folder ? `${filter.folder}/` : "";
  const inside = files.filter((f) => f.startsWith(prefix));

  const folders = [
    ...new Set(
      inside
        .map((f) => f.slice(prefix.length))
        .filter((rest) => rest.includes("/"))
        .map((rest) => rest.slice(0, rest.indexOf("/"))),
    ),
  ].sort();

  const here = inside
    .map((f) => ({ path: f, rest: f.slice(prefix.length) }))
    .filter((f) => !f.rest.includes("/"))
    .map(({ path, rest }) => {
      const ext = rest.slice(rest.lastIndexOf(".") + 1);
      return { path, name: rest, kind: kindOfExt(ext), imported: IMPORTED_EXTS.includes(ext) };
    })
    .filter((f) => filter.kinds.length === 0 || filter.kinds.includes(f.kind))
    .filter((f) => !filter.search || f.name.toLowerCase().includes(filter.search.toLowerCase()))
    .sort((a, b) => a.name.localeCompare(b.name));

  return { folders, files: here };
}

/** The breadcrumb trail. ⚠ **Every crumb is a target** — Unreal's, and the reason it is a trail. */
export function crumbs(folder: string): { label: string; path: string }[] {
  const out = [{ label: "Content", path: "" }];
  let acc = "";
  for (const part of folder.split("/").filter(Boolean)) {
    acc = acc ? `${acc}/${part}` : part;
    out.push({ label: part, path: acc });
  }
  return out;
}

/** Every folder in the root, for the sources tree. ⚠ Folders only — a file never appears in the tree. */
export function folderTree(files: string[]): string[] {
  const set = new Set<string>();
  for (const f of files) {
    const parts = f.split("/");
    parts.pop();
    let acc = "";
    for (const p of parts) {
      acc = acc ? `${acc}/${p}` : p;
      set.add(acc);
    }
  }
  return [...set].sort();
}

// ---------------------------------------------------------------------------------------------
// The create menu — two tiers, and that split is the whole of its design
// ---------------------------------------------------------------------------------------------

/**
 * ⚠ **Unreal's menu is four sections, and the interesting part is the split**: a short flat
 * `CREATE BASIC ASSET` list above a long categorized `CREATE ADVANCED ASSET` one. ▶ **A developer who
 * wants the common thing never opens a submenu**, and nothing about their experience paid for the
 * developer who wants a tag set.
 */
export interface CreateEntry {
  id: string;
  label: string;
  icon: string;
  doc: string;
}

export const CREATE_MENU: { basic: CreateEntry[]; advanced: { group: string; items: CreateEntry[] }[] } = {
  basic: [
    { id: "cvs", label: "Schematic", icon: "schematic", doc: "An object, its hooks and its dials." },
    { id: "cvcurve", label: "Curve table", icon: "curve", doc: "Named curves over one domain." },
    { id: "cvunlock", label: "Unlock table", icon: "unlock", doc: "The project's progression vocabulary." },
  ],
  advanced: [
    {
      group: "Structure",
      items: [
        { id: "cvspine", label: "Spine template", icon: "spine", doc: "Slots along a progression axis." },
        { id: "cvstate", label: "State graph", icon: "state", doc: "Settings of one world variable." },
      ],
    },
    {
      group: "Vocabulary",
      items: [{ id: "cvtags", label: "Tag set", icon: "tags", doc: "Names the project tags surfaces with." }],
    },
  ],
};

/** Draw the create menu. ⚠ **Every leaf carries an icon and a one-line description.** */
export function createMenu(): string {
  const leaf = (i: CreateEntry) =>
    `<button class="cv-mitem" data-create="${i.id}" title="${esc(i.doc)}">` +
    `${icon(i.icon, 15)}<span class="cv-mlabel">${esc(i.label)}</span>` +
    `<span class="cv-mdoc">${esc(i.doc)}</span></button>`;
  return (
    `<div class="cv-menu-pop" role="menu">` +
    `<div class="cv-msec">Create basic asset</div>` +
    CREATE_MENU.basic.map(leaf).join("") +
    `<div class="cv-msec">Create advanced asset</div>` +
    CREATE_MENU.advanced
      .map(
        (g) =>
          `<div class="cv-mgroup">${esc(g.group)}</div>` + g.items.map(leaf).join(""),
      )
      .join("") +
    `<div class="cv-msec">Get content</div>` +
    `<button class="cv-mitem" data-create="import" title="Bring in a mesh, or convert a spreadsheet into a curve table">` +
    `${icon("mesh", 15)}<span class="cv-mlabel">Import…</span>` +
    `<span class="cv-mdoc">Optional — every kind can be created empty instead</span></button>` +
    `</div>`
  );
}

// ---------------------------------------------------------------------------------------------
// The browser
// ---------------------------------------------------------------------------------------------

/** A tile's picture. ▶ **Where the asset can be drawn, draw it**; the kind icon is the fallback. */
function thumbnail(e: Entry, thumbs: Record<string, string>): string {
  return thumbs[e.path] ?? iconForPath(e.path, 34);
}

export interface BrowserState extends ContentFilter {
  /** Docked into the layout, rather than overlaying the stage as a drawer. */
  docked: boolean;
}

/**
 * The whole browser: sources tree, toolbar, breadcrumb, filters, tiles, footer.
 *
 * ⚠ **An empty folder is a teaching surface** — Unreal's reads *"Drop files here or right click to
 * create content."*, which removes the only question a new developer has, in the one place they are
 * guaranteed to be looking.
 */
export function drawBrowser(
  files: string[],
  state: BrowserState,
  thumbs: Record<string, string> = {},
): string {
  const { folders, files: here } = contentAt(files, state);
  const tree = folderTree(files);

  const trail = crumbs(state.folder)
    .map(
      (c, i, all) =>
        `<button class="cv-crumb" data-folder="${esc(c.path)}">${esc(c.label)}</button>` +
        (i < all.length - 1 ? `<span class="cv-csep">›</span>` : ""),
    )
    .join("");

  const chips = CONTENT_KINDS.map(
    (k) =>
      `<button class="cv-chip${state.kinds.includes(k.label) ? " is-on" : ""}" ` +
      `data-kind="${k.label}">${k.label}</button>`,
  ).join("");

  const treeRows = tree
    .map(
      (f) =>
        `<button class="cv-trow${f === state.folder ? " is-selected" : ""}" data-folder="${esc(f)}" ` +
        `style="padding-left:${6 + f.split("/").length * 10}px">` +
        `${icon("folder", 13)}<span>${esc(f.split("/").pop()!)}</span></button>`,
    )
    .join("");

  const tiles =
    folders
      .map(
        (f) =>
          `<button class="cv-tile is-folder" data-folder="${esc(
            state.folder ? `${state.folder}/${f}` : f,
          )}"><span class="cv-tpic">${icon("folder", 34)}</span>` +
          `<span class="cv-tname">${esc(f)}</span><span class="cv-tkind">Folder</span></button>`,
      )
      .join("") +
    here
      .map(
        (e) =>
          `<button class="cv-tile" data-file="${esc(e.path)}" ` +
          `title="${esc(e.name)} — ${esc(e.kind)}${e.imported ? " (imported; opens outside the editor)" : ""}">` +
          `<span class="cv-tpic">${thumbnail(e, thumbs)}</span>` +
          `<span class="cv-tname">${esc(e.name)}</span>` +
          `<span class="cv-tkind${e.imported ? " is-imported" : ""}">${esc(e.kind)}</span></button>`,
      )
      .join("");

  const body =
    tiles ||
    `<div class="cv-browser-empty">Drop files here, or right-click to create content.</div>`;

  return (
    `<div class="cv-browser">` +
    `<div class="cv-sources">` +
    `<div class="cv-shead">Content</div>` +
    `<button class="cv-trow${state.folder === "" ? " is-selected" : ""}" data-folder="">` +
    `${icon("folder", 13)}<span>All</span></button>` +
    treeRows +
    `</div>` +
    `<div class="cv-assets">` +
    `<div class="cv-btoolbar">` +
    `<button class="cv-bbtn is-primary" data-open-create="1">${icon("add", 13)}Add</button>` +
    `<button class="cv-bbtn" data-create="import">${icon("mesh", 13)}Import</button>` +
    `<button class="cv-bbtn" data-tool="saveall">${icon("saveall", 13)}Save All</button>` +
    `<div class="cv-crumbs">${trail}</div>` +
    `<button class="cv-bbtn" data-dock-browser="1">${state.docked ? "Undock" : "Dock in Layout"}</button>` +
    `</div>` +
    `<div class="cv-bfilters">${chips}` +
    `<input class="cv-search" placeholder="Search ${esc(state.folder || "Content")}" ` +
    `value="${esc(state.search)}"/></div>` +
    `<div class="cv-tiles">${body}</div>` +
    `<div class="cv-bfoot">${folders.length + here.length} item` +
    `${folders.length + here.length === 1 ? "" : "s"}</div>` +
    `</div></div>`
  );
}

/** The browser's stylesheet. ⚠ Nothing here picks a hex. */
export function browserStyles(): string {
  const V = (n: string) => `var(--cv-${n})`;
  return `
.cv-browser { display: flex; min-height: 0; height: 100%; }
.cv-sources { width: 190px; border-right: 1px solid ${V("line")}; overflow: auto; padding: 4px 0;
  flex: 0 0 auto; }
.cv-shead { padding: 4px 10px; color: ${V("muted")}; font-size: 10px; text-transform: uppercase;
  letter-spacing: .09em; }
.cv-trow { display: flex; align-items: center; gap: 6px; width: 100%; background: none; border: 0;
  color: ${V("text")}; font: inherit; cursor: pointer; padding: 3px 8px; text-align: left; }
.cv-trow:hover { background: ${V("raised")}; }
.cv-trow.is-selected { background: ${V("selected")}; box-shadow: inset 2px 0 0 ${V("accent")}; }
.cv-trow:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: -2px; }

.cv-assets { flex: 1 1 auto; display: flex; flex-direction: column; min-width: 0; min-height: 0; }
.cv-btoolbar { display: flex; align-items: center; gap: 6px; padding: 5px 8px;
  border-bottom: 1px solid ${V("line")}; }
.cv-bbtn { display: inline-flex; align-items: center; gap: 5px; background: ${V("raised")};
  border: 1px solid ${V("line")}; color: ${V("text")}; font: inherit; cursor: pointer;
  padding: 3px 9px; border-radius: ${V("radius")}; white-space: nowrap; }
.cv-bbtn:hover { border-color: ${V("accent")}; }
.cv-bbtn:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: 1px; }
.cv-bbtn.is-primary { border-color: ${V("accent")}; }

.cv-crumbs { display: flex; align-items: center; gap: 3px; margin-left: 8px; overflow: hidden; }
.cv-crumb { background: none; border: 0; color: ${V("muted")}; font: inherit; cursor: pointer;
  padding: 2px 4px; border-radius: ${V("radius")}; }
.cv-crumb:hover { color: ${V("accent")}; background: ${V("raised")}; }
.cv-csep { color: ${V("muted")}; opacity: .6; }

.cv-bfilters { display: flex; align-items: center; gap: 5px; padding: 6px 8px;
  border-bottom: 1px solid ${V("line")}; flex-wrap: wrap; }
.cv-bfilters .cv-search { flex: 1 1 160px; min-width: 120px; }

.cv-tiles { flex: 1 1 auto; overflow: auto; padding: 10px;
  display: grid; grid-template-columns: repeat(auto-fill, minmax(108px, 1fr)); gap: 10px;
  align-content: start; }
/* WARN **A tile answers "what is this" with no hover and no filter** — Unreal's kind strip. */
.cv-tile { display: flex; flex-direction: column; align-items: stretch; gap: 0;
  background: ${V("raised")}; border: 1px solid ${V("line")}; border-radius: ${V("radius")};
  color: ${V("text")}; font: inherit; cursor: pointer; overflow: hidden; padding: 0; }
.cv-tile:hover { border-color: ${V("accent")}; }
.cv-tile:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: 1px; }
.cv-tpic { display: flex; align-items: center; justify-content: center; height: 58px;
  color: ${V("muted")}; background: ${V("bg")}; }
.cv-tile:hover .cv-tpic { color: ${V("text")}; }
.cv-tname { padding: 5px 6px 3px; font-size: 11px; line-height: 1.3; text-align: center;
  display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
  overflow-wrap: anywhere; }
.cv-tkind { font-size: 9px; letter-spacing: .07em; text-transform: uppercase; text-align: center;
  padding: 2px 4px; background: ${V("selected")}; color: ${V("muted")}; }
.cv-tkind.is-imported { opacity: .7; font-style: italic; }
.cv-tile.is-folder .cv-tkind { background: transparent; }
.cv-browser-empty { grid-column: 1 / -1; color: ${V("muted")}; text-align: center; padding: 28px 8px; }
.cv-bfoot { padding: 4px 10px; border-top: 1px solid ${V("line")}; color: ${V("muted")};
  font-size: 11px; }

/* The create menu — two tiers, each leaf iconed and described. */
.cv-menu-pop { position: absolute; z-index: 40; min-width: 300px; background: ${V("panel")};
  border: 1px solid ${V("line")}; border-radius: ${V("radius")}; padding: 5px;
  box-shadow: 0 8px 26px rgb(0 0 0 / .45); }
.cv-msec { padding: 7px 9px 3px; color: ${V("muted")}; font-size: 9.5px; text-transform: uppercase;
  letter-spacing: .1em; border-top: 1px solid ${V("line")}; margin-top: 3px; }
.cv-msec:first-child { border-top: 0; margin-top: 0; }
.cv-mgroup { padding: 4px 9px 2px; color: ${V("muted")}; font-size: 10.5px; font-weight: 600; }
.cv-mitem { display: grid; grid-template-columns: auto 1fr; gap: 2px 9px; width: 100%;
  background: none; border: 0; color: ${V("text")}; font: inherit; cursor: pointer;
  padding: 5px 9px; border-radius: ${V("radius")}; text-align: left; align-items: center; }
.cv-mitem:hover { background: ${V("selected")}; }
.cv-mitem:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: -2px; }
.cv-mlabel { font-size: 12.5px; }
.cv-mdoc { grid-column: 2; color: ${V("muted")}; font-size: 11px; }
.cv-icon { flex: 0 0 auto; vertical-align: middle; }
`;
}
