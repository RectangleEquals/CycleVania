/**
 * **The tag editor** — the project's tag vocabulary.
 *
 * ⚠ **`/Core/Tag` is *"a dotted hierarchical label, picked rather than typed"***, and that phrase is the
 * whole specification for this surface: ▶ **a tag set exists so that an `Array<Tag>` field can be a
 * picker instead of a text box.** Without a vocabulary there is nothing to pick from, and the field
 * degrades to free text — which is how a project ends up with `stone`, `Stone` and `stoen`.
 *
 * `10-editor.md` §2.
 */

const esc = (s: string) =>
  s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);

export interface TagDef {
  /** The dotted path — `surface.stone.wet`. */
  name: string;
  doc?: string;
  /** How many objects or rules reference it. ⚠ Zero is a lint, not an error. */
  uses?: number;
}

export interface TagNode {
  segment: string;
  path: string;
  doc?: string;
  uses?: number;
  /** ⚠ A node that exists only because something deeper does — never declared on its own. */
  implied: boolean;
  children: TagNode[];
}

/**
 * Build the tree a dotted vocabulary implies.
 *
 * ⚠ **An intermediate segment is *implied*, not declared.** `surface.stone.wet` creates `surface` and
 * `surface.stone` as structure — ▶ **and the editor must say which is which**, because deleting a
 * declared tag and pruning an implied one are different actions with different consequences.
 */
export function tagTree(tags: TagDef[]): TagNode[] {
  const roots: TagNode[] = [];
  const byPath = new Map<string, TagNode>();

  for (const t of [...tags].sort((a, b) => a.name.localeCompare(b.name))) {
    const parts = t.name.split(".");
    let acc = "";
    let level = roots;
    parts.forEach((seg, i) => {
      acc = acc ? `${acc}.${seg}` : seg;
      let node = byPath.get(acc);
      if (!node) {
        node = { segment: seg, path: acc, implied: true, children: [] };
        byPath.set(acc, node);
        level.push(node);
      }
      if (i === parts.length - 1) {
        node.implied = false;
        node.doc = t.doc;
        node.uses = t.uses;
      }
      level = node.children;
    });
  }
  return roots;
}

/** Every declared tag, flattened — what a picker offers. */
export function declared(tags: TagDef[]): string[] {
  return tags.map((t) => t.name).sort();
}

/** Draw the tag tree. */
export function drawTags(tags: TagDef[], selected?: string): string {
  const row = (n: TagNode, depth: number): string =>
    `<div class="cv-tagrow${n.path === selected ? " is-selected" : ""}` +
    `${n.implied ? " is-implied" : ""}" data-tag="${esc(n.path)}" ` +
    `style="padding-left:${8 + depth * 16}px" title="${esc(n.doc ?? n.path)}">` +
    `<span class="cv-tagseg">${esc(n.segment)}</span>` +
    (n.implied
      ? `<span class="cv-tagnote">implied</span>`
      : // ⚠ **Zero uses is a lint, not an error** — a vocabulary may run ahead of the content.
        `<span class="cv-tagnote${n.uses === 0 ? " cv-warn" : ""}">` +
        `${n.uses === undefined ? "" : `${n.uses} use${n.uses === 1 ? "" : "s"}`}</span>`) +
    `</div>` +
    n.children.map((c) => row(c, depth + 1)).join("");

  const tree = tagTree(tags);
  return (
    `<div class="cv-tagwrap">` +
    `<div class="cv-tagbar">` +
    `<button class="cv-bbtn is-primary" data-addtag="1">+ Tag</button>` +
    `<span class="cv-dim">${declared(tags).length} declared</span>` +
    `</div>` +
    (tree.length
      ? tree.map((n) => row(n, 0)).join("")
      : // ⚠ **The empty state names what this is for** — §9d.
        `<div class="cv-empty">No tags yet. A tag set is what lets a <b>Tag</b> field be a picker ` +
        `rather than a text box — <b>+ Tag</b> adds one.</div>`) +
    `</div>`
  );
}

export function tagStyles(): string {
  const V = (n: string) => `var(--cv-${n})`;
  return `
.cv-tagwrap { padding: 8px 0; }
.cv-tagbar { display: flex; align-items: center; gap: 10px; padding: 0 10px 10px; }
.cv-tagrow { display: flex; align-items: center; gap: 8px; padding: 3px 10px; cursor: pointer;
  font-size: 12.5px; }
.cv-tagrow:hover { background: ${V("raised")}; }
.cv-tagrow.is-selected { background: ${V("selected")}; box-shadow: inset 2px 0 0 ${V("accent")}; }
/* WARN An implied segment is structure, not a declaration — deleting one is a different action. */
.cv-tagrow.is-implied .cv-tagseg { color: ${V("muted")}; font-style: italic; }
.cv-tagnote { margin-left: auto; color: ${V("muted")}; font-size: 10.5px; }
`;
}
