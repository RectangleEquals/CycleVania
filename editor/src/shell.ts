/**
 * **Shared editor chrome** — the stylesheet, and the Content panel's shape.
 *
 * ⚠ **The topbar and navigator that used to live here are gone.** They were M20a's arrangement, and
 * `10-editor.md` §2 replaced the architecture behind it rather than the styling on top: a `Views`
 * picker is not a placeholder for a dock layout, it is a different answer to *why a view is on screen*.
 * ▶ The frame is `frame.ts`; what stays here is the shared stylesheet and the Content panel.
 *
 * # Every colour comes from the four parameters
 *
 * ⚠ **Nothing here picks a hex.** A component that computed its own colour would be a second theme, and
 * the second one goes stale the first time the first one moves.
 */

import { pinColour } from "./pins.ts";

const V = (name: string) => `var(--cv-${name})`;

/** The stylesheet, written once against the custom properties the theme installs. */
export function shellStyles(): string {
  return `
* { box-sizing: border-box; }
html, body, #app { height: 100%; margin: 0; }
body {
  background: ${V("bg")}; color: ${V("text")};
  font: 13px ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
}
#app { display: grid; grid-template-rows: auto 1fr; height: 100vh; }

.cv-topbar {
  display: flex; align-items: center; gap: 10px; padding: 7px 12px;
  background: ${V("panel")}; border-bottom: 1px solid ${V("line")};
}
.cv-brand { font-weight: 600; color: ${V("accent")}; letter-spacing: .02em; }
.cv-version { color: ${V("muted")}; font: 11px ui-monospace, monospace; }
.cv-tabs { display: flex; gap: 2px; margin-left: auto; }

/* ⚠ States, not decoration. A control that does not answer the pointer reads as disabled. */
.cv-tab, .cv-btn {
  background: ${V("raised")}; color: ${V("text")}; border: 1px solid ${V("line")};
  border-radius: ${V("radius")}; padding: 4px 10px; font: inherit; cursor: pointer;
}
.cv-tab:hover, .cv-btn:hover { border-color: ${V("accent")}; }
.cv-tab:focus-visible, .cv-btn:focus-visible { outline: 2px solid ${V("accent")}; outline-offset: 1px; }
.cv-tab.is-active { background: ${V("selected")}; border-color: ${V("accent")}; }
.cv-btn:disabled { opacity: .45; cursor: default; border-color: ${V("line")}; }

.cv-body { display: grid; grid-template-columns: 232px 1fr 296px; min-height: 0; }
.cv-nav, .cv-inspector { background: ${V("panel")}; overflow: auto; padding: 8px; }
.cv-nav { border-right: 1px solid ${V("line")}; }
.cv-inspector { border-left: 1px solid ${V("line")}; }
.cv-stage { position: relative; overflow: auto; min-width: 0; padding: 16px; background: ${V("bg")}; }

.cv-h { margin: 8px 4px 4px; color: ${V("muted")}; font-size: 10px;
        text-transform: uppercase; letter-spacing: .09em; }
.cv-row {
  display: flex; gap: 6px; align-items: baseline; padding: 3px 6px;
  border-radius: ${V("radius")}; cursor: pointer; white-space: nowrap;
  overflow: hidden; text-overflow: ellipsis;
}
.cv-row:hover { background: ${V("raised")}; }
.cv-row.is-selected { background: ${V("selected")}; box-shadow: inset 2px 0 0 ${V("accent")}; }
.cv-row .cv-hint { color: ${V("muted")}; font-size: 11px; margin-left: auto; }

.cv-card { background: ${V("panel")}; border: 1px solid ${V("line")};
           border-radius: ${V("radius")}; padding: 10px; margin: 0 0 16px; }
.cv-card > h3 { margin: 0 0 2px; font-size: 11px; color: ${V("muted")};
                text-transform: uppercase; letter-spacing: .08em; font-weight: 600; }
.cv-card > p.cv-note { margin: 0 0 10px; color: ${V("muted")}; font-size: 12px; max-width: 68ch; }

table.cv { border-collapse: collapse; width: 100%; font-size: 12.5px; }
table.cv th { text-align: left; padding: 5px 8px; border-bottom: 1px solid ${V("line")};
              font-size: 10px; color: ${V("muted")}; text-transform: uppercase; letter-spacing: .07em; }
table.cv td { padding: 5px 8px; border-bottom: 1px solid ${V("line")}; }
table.cv tr:hover td { background: ${V("raised")}; }
.cv-mono { font-family: ui-monospace, "Cascadia Code", Consolas, monospace; }
.cv-dim { color: ${V("muted")}; }
.cv-ok { color: ${V("ok")}; } .cv-warn { color: ${V("warn")}; } .cv-err { color: ${V("err")}; }

/* ⚠ The breadcrumb is Unreal's: where you are, and every step back to the root is a target. */
.cv-crumbs { display: flex; gap: 4px; align-items: center; font-size: 11px; color: ${V("muted")};
             padding: 2px 4px 8px; flex-wrap: wrap; }
.cv-crumb { cursor: pointer; }
.cv-crumb:hover { color: ${V("accent")}; }

/* Stacking filters — several may be on at once, each toggled alone. */
.cv-filters { display: flex; gap: 4px; flex-wrap: wrap; padding: 0 4px 8px; }
.cv-chip {
  border: 1px solid ${V("line")}; border-radius: 999px; padding: 2px 9px;
  font-size: 11px; color: ${V("muted")}; cursor: pointer; background: transparent;
}
.cv-chip:hover { border-color: ${V("accent")}; color: ${V("text")}; }
.cv-chip.is-on { background: ${V("selected")}; border-color: ${V("accent")}; color: ${V("text")}; }

.cv-search {
  width: 100%; background: ${V("raised")}; color: ${V("text")};
  border: 1px solid ${V("line")}; border-radius: ${V("radius")}; padding: 4px 8px; font: inherit;
}
.cv-search:focus { outline: none; border-color: ${V("accent")}; }
.cv-empty { color: ${V("muted")}; font-size: 12px; padding: 6px; }
`;
}

// ---------------------------------------------------------------------------------------------
// P06 — the Content panel, Unreal's Content Browser shape
// ---------------------------------------------------------------------------------------------

/** Every extension the content root may hold, and what to call it. */
export const CONTENT_KINDS: { ext: string; label: string }[] = [
  { ext: "cvs", label: "Schematic" },
  { ext: "cvspine", label: "Spine" },
  { ext: "cvstate", label: "State" },
  { ext: "cvcurve", label: "Curve" },
  { ext: "cvunlock", label: "Unlocks" },
  { ext: "cvtags", label: "Tags" },
];

/** How the Content panel is filtered. */
export interface ContentFilter {
  /** ⚠ **Several at once**, each toggled alone — Unreal's filters stack. */
  kinds: string[];
  /** The folder currently open, `""` for the root. */
  folder: string;
  /** Free text, matched against the name. */
  search: string;
}

/** Split content paths into the folders under the current one, and the files in it. */
export function contentAt(files: string[], filter: ContentFilter): {
  folders: string[];
  files: { path: string; name: string; kind: string }[];
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
      const kind = CONTENT_KINDS.find((k) => k.ext === ext)?.label ?? ext;
      return { path, name: rest, kind };
    })
    .filter((f) => filter.kinds.length === 0 || filter.kinds.includes(f.kind))
    .filter((f) => !filter.search || f.name.toLowerCase().includes(filter.search.toLowerCase()))
    .sort((a, b) => a.name.localeCompare(b.name));

  return { folders, files: here };
}

/** The breadcrumb trail for a folder. */
export function crumbs(folder: string): { label: string; path: string }[] {
  const out = [{ label: "Content", path: "" }];
  let acc = "";
  for (const part of folder.split("/").filter(Boolean)) {
    acc = acc ? `${acc}/${part}` : part;
    out.push({ label: part, path: acc });
  }
  return out;
}

/** Draw the Content panel. */
export function drawContent(files: string[], filter: ContentFilter): string {
  const { folders, files: here } = contentAt(files, filter);

  const trail = crumbs(filter.folder)
    .map((c, i, all) => {
      const sep = i < all.length - 1 ? `<span>/</span>` : "";
      return `<span class="cv-crumb" data-folder="${c.path}">${c.label}</span>${sep}`;
    })
    .join("");

  const chips = CONTENT_KINDS.map(
    (k) =>
      `<button class="cv-chip${filter.kinds.includes(k.label) ? " is-on" : ""}" ` +
      `data-kind="${k.label}">${k.label}</button>`,
  ).join("");

  const rows =
    folders
      .map(
        (f) =>
          `<div class="cv-row" data-folder="${filter.folder ? `${filter.folder}/${f}` : f}">` +
          `<span class="cv-dim">▸</span><span>${f}</span></div>`,
      )
      .join("") +
    here
      .map(
        (f) =>
          `<div class="cv-row" data-file="${f.path}"><span>${f.name}</span>` +
          `<span class="cv-hint">${f.kind}</span></div>`,
      )
      .join("");

  return (
    `<div class="cv-crumbs">${trail}</div>` +
    `<div class="cv-filters">${chips}</div>` +
    `<input class="cv-search" placeholder="Search content" value="${filter.search}"/>` +
    (rows || `<div class="cv-empty">Nothing here.</div>`)
  );
}

/** A pin swatch, for an inspector row that names a type. */
export function pinSwatch(type: string): string {
  return (
    `<span style="display:inline-block;width:9px;height:9px;border-radius:50%;` +
    `background:${pinColour(type)};margin-right:6px;vertical-align:middle"></span>`
  );
}
