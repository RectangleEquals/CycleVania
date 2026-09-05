/**
 * **Shared editor chrome** — the stylesheet, and the Content panel's shape.
 *
 * ⚠ **The Content panel moved to `browser.ts`, and the topbar and navigator are gone.** They were M20a's arrangement, and
 * `10-editor.md` §2 replaced the architecture behind it rather than the styling on top: a `Views`
 * picker is not a placeholder for a dock layout, it is a different answer to *why a view is on screen*.
 * ▶ The frame is `frame.ts` and the one Content Browser is `browser.ts`; what stays here is the shared
 * stylesheet the views draw against.
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

/** A pin swatch, for an inspector row that names a type. */
export function pinSwatch(type: string): string {
  return (
    `<span style="display:inline-block;width:9px;height:9px;border-radius:50%;` +
    `background:${pinColour(type)};margin-right:6px;vertical-align:middle"></span>`
  );
}
