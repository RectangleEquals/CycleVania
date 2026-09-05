/**
 * **The icon set.**
 *
 * ⚠ **A first build drew the chrome in text and symbols only**, and the result was legible and hard to
 * read — every control cost a moment of parsing. ▶ **That is a §9d failure rather than a polish one**:
 * hidden complexity is about what a developer has to *work out*, and a row of bare words is a row of
 * small translations.
 *
 * # The rules — `10-editor.md` §9e
 *
 * - **An icon never travels alone.** Icon *plus* label wherever there is room. ⚠ An icon that has to be
 *   explained is a label with extra steps.
 * - **One icon per concept, everywhere that concept appears.** A schematic's mark is the same in the
 *   browser tile, the asset tab and the Details subject line. ⚠ **A concept with two marks has none.**
 * - **Monochrome, drawn in `currentColor`**, so §9a's rule survives: the theme's four parameters still
 *   own every colour.
 * - **Shape carries the category.** Content kinds share a family, layer views another, verbs a third.
 * - ⚠ **Inline SVG — no icon font, no sprite sheet.** A font that fails to load leaves boxes, and a
 *   sheet is a second asset to keep in step with a palette that already moves.
 */

/** Every icon, as a path body drawn on a 16×16 grid with `currentColor`. */
const PATHS: Record<string, string> = {
  // ── content kinds: a document silhouette, differing in what is on the page ──────────────
  // ⚠ **The family is the outline**: a developer reads "an asset" before they read "which".
  schematic:
    `<path d="M3 1.5h6.5L13 5v9.5H3z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<circle cx="6" cy="8" r="1.4" fill="currentColor"/><circle cx="10" cy="11" r="1.4" fill="currentColor"/>` +
    `<path d="M7.4 8.6 8.6 10.4" stroke="currentColor" stroke-width="1.2"/>`,
  spine:
    `<path d="M3 1.5h6.5L13 5v9.5H3z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M5 11.5h6" stroke="currentColor" stroke-width="1.2"/>` +
    `<circle cx="5.5" cy="11.5" r="1.1" fill="currentColor"/><circle cx="8" cy="11.5" r="1.1" fill="currentColor"/>` +
    `<circle cx="10.5" cy="11.5" r="1.1" fill="currentColor"/>`,
  state:
    `<path d="M3 1.5h6.5L13 5v9.5H3z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<circle cx="6" cy="9" r="1.5" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<circle cx="10.2" cy="9" r="1.5" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M7.6 9h1.1" stroke="currentColor" stroke-width="1.2"/>`,
  curve:
    `<path d="M3 1.5h6.5L13 5v9.5H3z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M5 12c2-.5 2.2-5 5.8-5.4" fill="none" stroke="currentColor" stroke-width="1.3"/>`,
  unlock:
    `<path d="M3 1.5h6.5L13 5v9.5H3z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M6.4 9V7.6a1.6 1.6 0 0 1 3.2 0" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<rect x="5.4" y="9" width="5.2" height="3.6" rx=".8" fill="currentColor"/>`,
  tags:
    `<path d="M3 1.5h6.5L13 5v9.5H3z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M5.2 9.2h3.4M5.2 11.4h5" stroke="currentColor" stroke-width="1.2"/>`,
  mesh:
    `<path d="M8 2.2 13.4 5.3v6.2L8 14.6 2.6 11.5V5.3z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M2.6 5.3 8 8.4l5.4-3.1M8 8.4v6.2" fill="none" stroke="currentColor" stroke-width="1.1"/>`,
  folder:
    `<path d="M2 4.2h4.3l1.2 1.6H14v7.4H2z" fill="none" stroke="currentColor" stroke-width="1.2"/>`,

  // ── layer views: a family of *stages*, from abstract graph to finished space ────────────
  mission:
    `<circle cx="3.6" cy="8" r="1.8" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<circle cx="12.4" cy="4.6" r="1.8" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<circle cx="12.4" cy="11.6" r="1.8" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M5.3 7.2 10.7 5.3M5.3 8.8l5.4 1.9" stroke="currentColor" stroke-width="1.2"/>`,
  skeleton:
    `<path d="M2.4 11.2V5.6l5.6-3 5.6 3v5.6l-5.6 3z" fill="none" stroke="currentColor" ` +
    `stroke-width="1.2" stroke-dasharray="2.4 1.6"/>`,
  // ⚠ **Volume and Geometry read alike at a glance and had to be pulled apart.** ▶ Volume is
  // *space that got carved* — a solid with an opening cut through it. Geometry is *what got built* —
  // the same solid, panelled. **The family still reads as one; the members no longer trade places.**
  volume:
    `<path d="M2.4 11.2V5.6l5.6-3 5.6 3v5.6l-5.6 3z" fill="currentColor" opacity=".18"/>` +
    `<path d="M2.4 11.2V5.6l5.6-3 5.6 3v5.6l-5.6 3z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M6 14.2V9.4a2 2 0 0 1 4 0v4.8" fill="none" stroke="currentColor" stroke-width="1.3"/>`,
  // ⚠ A hex fallback here failed the `currentColor` test on its first run — ▶ **which is
  // exactly what that assertion is for**: an icon carrying its own colour is a second palette.
  geometry:
    `<path d="M2.4 11.2V5.6l5.6-3 5.6 3v5.6l-5.6 3z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M2.4 5.6 8 8.6l5.6-3M8 8.6v5.9" stroke="currentColor" stroke-width="1.1"/>` +
    `<path d="M2.4 5.6 8 14.5 13.6 5.6M2.4 11.2 8 8.6l5.6 2.6" fill="none" stroke="currentColor" ` +
    `stroke-width=".9" opacity=".75"/>`,
  final:
    `<path d="M2.4 11.2V5.6l5.6-3 5.6 3v5.6l-5.6 3z" fill="currentColor" opacity=".55"/>` +
    `<path d="M2.4 11.2V5.6l5.6-3 5.6 3v5.6l-5.6 3z" fill="none" stroke="currentColor" stroke-width="1.2"/>`,

  // ── verbs ───────────────────────────────────────────────────────────────────────────────
  save:
    `<path d="M2.6 2.6h8.2L13.4 5.2v8.2H2.6z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M5.4 2.6v3.6h5V2.6M5 13.4V9.6h6v3.8" fill="none" stroke="currentColor" stroke-width="1.2"/>`,
  saveall:
    `<path d="M4.6 4.6h7.2L14 6.8v6.6H4.6z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M2 11.4V2.6h8" fill="none" stroke="currentColor" stroke-width="1.2"/>`,
  add: `<path d="M8 3.4v9.2M3.4 8h9.2" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>`,
  generate:
    `<path d="M4.4 2.8 12.8 8l-8.4 5.2z" fill="currentColor"/>`,
  compile:
    `<path d="M8 1.8a6.2 6.2 0 1 1-6.2 6.2" fill="none" stroke="currentColor" stroke-width="1.3"/>` +
    `<path d="M5.2 8.2 7.3 10.4 11 5.8" fill="none" stroke="currentColor" stroke-width="1.4"/>`,

  // ── docks ───────────────────────────────────────────────────────────────────────────────
  components:
    `<rect x="6" y="1.8" width="4" height="4" rx=".8" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<rect x="1.8" y="10.2" width="4" height="4" rx=".8" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<rect x="10.2" y="10.2" width="4" height="4" rx=".8" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M8 5.8v2.4M8 8.2H3.8v2M8 8.2h4.2v2" fill="none" stroke="currentColor" stroke-width="1.1"/>`,
  viewport:
    `<rect x="1.8" y="3" width="12.4" height="10" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M5 10.6 7.4 7.4l1.8 2.2 1.4-1.6 2 2.6z" fill="currentColor" opacity=".55"/>` +
    `<circle cx="5.2" cy="5.8" r="1" fill="currentColor"/>`,
  setup:
    `<path d="M8 2.4v2.2M8 11.4v2.2M2.4 8h2.2M11.4 8h2.2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>` +
    `<circle cx="8" cy="8" r="3.2" fill="none" stroke="currentColor" stroke-width="1.2"/>`,
  outline:
    `<path d="M2.6 4h2.2M2.6 8h2.2M2.6 12h2.2" stroke="currentColor" stroke-width="1.3"/>` +
    `<path d="M7 4h6.4M7 8h6.4M7 12h6.4" stroke="currentColor" stroke-width="1.3"/>`,
  details:
    `<rect x="2.4" y="2.6" width="11.2" height="10.8" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M5 6.2h6M5 9h6M5 11.4h3.4" stroke="currentColor" stroke-width="1.2"/>`,
  content:
    `<rect x="2.2" y="2.6" width="4.6" height="4.6" rx=".8" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<rect x="9.2" y="2.6" width="4.6" height="4.6" rx=".8" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<rect x="2.2" y="9" width="4.6" height="4.6" rx=".8" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<rect x="9.2" y="9" width="4.6" height="4.6" rx=".8" fill="none" stroke="currentColor" stroke-width="1.2"/>`,
  findings:
    `<path d="M8 2.2 14.4 13.4H1.6z" fill="none" stroke="currentColor" stroke-width="1.2"/>` +
    `<path d="M8 6.4v3.2" stroke="currentColor" stroke-width="1.4"/><circle cx="8" cy="11.5" r=".9" fill="currentColor"/>`,
};

/** Which icon a content extension gets. ⚠ One mark per concept, and this is where that is decided. */
export const ICON_FOR_EXT: Record<string, string> = {
  cvs: "schematic",
  cvspine: "spine",
  cvstate: "state",
  cvcurve: "curve",
  cvunlock: "unlock",
  cvtags: "tags",
  glb: "mesh",
  gltf: "mesh",
  obj: "mesh",
};

/** ⚠ Named, never numbered — `L3` means nothing to anyone who has not read the pipeline. */
export const LAYER_ICON: Record<string, string> = {
  L1: "mission",
  L2: "skeleton",
  L3: "volume",
  L4: "geometry",
  L5: "final",
};

export const iconNames = (): string[] => Object.keys(PATHS);
export const hasIcon = (name: string): boolean => name in PATHS;

/**
 * One icon, at `size` px.
 *
 * ⚠ `aria-hidden`, because **the label beside it is the accessible name** — an icon that travels alone
 * would need its own, and §9e says it never does.
 */
export function icon(name: string, size = 14): string {
  const body = PATHS[name];
  if (!body) return "";
  return (
    `<svg class="cv-icon" width="${size}" height="${size}" viewBox="0 0 16 16" ` +
    `fill="none" aria-hidden="true" focusable="false">${body}</svg>`
  );
}

/** The icon for a content path, by extension. Falls back to a plain document. */
export function iconForPath(path: string, size = 14): string {
  const ext = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
  return icon(ICON_FOR_EXT[ext] ?? "tags", size);
}
