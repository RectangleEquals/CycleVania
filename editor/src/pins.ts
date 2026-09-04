/**
 * **Pin colour by type, node colour by category** — Unreal's conventions, taken wholesale.
 *
 * ⚠ **`exec` is white and unmistakable.** Every other pin takes a colour from its type. This is not
 * decoration: it makes the connection rule *visible before a wire is tried*. `10-editor.md` §5 says
 * *impossible* means the wire does not draw — colour is what tells a developer **why** without their
 * having to attempt it, and a `Kind<T>` pin that will never meet a `Ref<T>` pin already looks unlike it.
 *
 * ▶ **The node's title bar carries the node's colour**, so category reads at a glance. The header is
 * the type; the body is the detail.
 *
 * # Colour is a hint, never the rule
 *
 * ⚠ **The rule is `graph.ts`'s `mayConnect`, and it still gives a reason.** A developer who cannot
 * distinguish two colours must be told *why* a wire refused — colour makes the common case instant, it
 * does not replace the explanation.
 */

/** ⚠ Execution is white, as in Blueprints. Nothing else may be. */
export const EXEC = "#e8e8e8";

/** Type colour, by the shape of the type rather than a list of every name. */
export function pinColour(type: string): string {
  if (type === "exec") return EXEC;

  // Primitives first — these are the ones a developer sees constantly.
  const primitives: Record<string, string> = {
    bool: "#8e3b3b",
    int: "#3f9c8f",
    float: "#5aab55",
    String: "#b34fa0",
  };
  const direct = primitives[type];
  if (direct) return direct;

  // ⚠ **Generic wrappers are coloured by the wrapper, not the parameter.** `Ref<Actor>` and `Ref<Item>`
  // connect to each other; `Kind<Actor>` and `Ref<Actor>` never do. Colouring by `T` would make the two
  // that *cannot* meet look identical and the two that *can* look different — exactly backwards.
  const generic = /^(Ref|Kind|Asset)<.+>$/.exec(type);
  if (generic) {
    switch (generic[1]) {
      case "Ref":
        return "#3d6ea8";
      case "Kind":
        return "#a87c3d";
      case "Asset":
        return "#7a5aab";
    }
  }

  // Structs and enums share a neutral, because inventing a colour per struct is how a palette rots.
  return "#7f8c99";
}

/**
 * The node's header colour, by category.
 *
 * ▶ **Unreal's coding**: blue for maths, green for flow control, orange for a call the project defines.
 * A developer scanning a dense graph reads the headers, not the labels.
 */
export function nodeColour(node: { op: string; shape?: string; category?: string }): string {
  const op = node.op.toLowerCase();
  if (op.startsWith("math.") || op.includes("/math")) return "#2f5d9e";
  if (["core.branch", "core.for_each", "core.for_range", "core.sequence"].some((c) => op.startsWith(c)))
    return "#3f7d4a";
  if (op.endsWith("#dial")) return "#8a6b2f";
  if (node.shape === "literal") return "#5a5f66";
  if (node.shape === "form") return "#6b4a8a";
  // A generated API call — the common case, and Unreal's neutral blue-grey.
  return "#3a4a5c";
}

/**
 * Does a pin's colour distinguish it from another's?
 *
 * ⚠ **Used by a test, not by the editor.** The claim *"colour makes a refusal predictable"* is only
 * true if the pairs that cannot connect actually look different — so it is asserted rather than hoped.
 */
export function looksDifferent(a: string, b: string): boolean {
  return pinColour(a) !== pinColour(b);
}
