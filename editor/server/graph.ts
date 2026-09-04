/**
 * **The graph editor's rules.**
 *
 * ⚠ **Connection rules live here and nowhere else.** The compiler deliberately does not type-check a
 * wire: *"Impossible is the editor's — a `Kind<T>` pin that will not connect to a `Ref<T>` pin is a
 * wire that does not draw."* So this is not a first line of defence with a compiler behind it. It is
 * the only line, and anything it permits reaches lowering unchecked.
 *
 * ▶ **Which is also why it is local.** A rule that runs while a wire follows the cursor cannot cross a
 * boundary; a round-trip per mouse move is not a design, it is a lag.
 *
 * # Three tiers, and only one of them is here
 *
 * | Tier | Who | What it looks like |
 * |---|---|---|
 * | **impossible** | ⚠ this file | the wire does not draw |
 * | **error** | the compiler | the compile stops |
 * | **warning** | the compiler | it does not |
 */

import { classAt, type ClassDef } from "./classes.ts";
import { palette, type PaletteNode } from "./palette.ts";
import type { Dial } from "./bindings.ts";

// ---------------------------------------------------------------------------------------------
// P02 / P02a — a node exists because the palette offers it
// ---------------------------------------------------------------------------------------------

/** ⚠ The fixed core set, from `07-authoring.md` §5. **There is no `While`** — every loop must be provably finite. */
export const CORE_NODES = [
  "Branch",
  "For Each",
  "For Range",
  "Expression",
  "Make Array",
  "Make Map",
  "Comment",
  "Group",
  "Reroute",
  "Send Message",
] as const;

/** Why a node could not be made. */
export class NotInPaletteError extends Error {}

/**
 * Make a node, **only** from something the palette offers.
 *
 * ⚠ **There is no text field to type a wrong name into.** That is the whole payoff of a visual
 * language: a misspelled op never becomes a document, so the compiler's *"no op named `array.is_emty`"*
 * finding exists only for schematics a script generated.
 */
export function nodeFor(op: string, extra: PaletteNode[] = []): PaletteNode {
  const found = [...palette(), ...extra].find((n) => n.op === op);
  if (!found) {
    throw new NotInPaletteError(
      `\`${op}\` is not in the palette — a node exists because the palette offers it, never because ` +
        `someone typed it`,
    );
  }
  return found;
}

/**
 * The palette a project actually sees: the generated artifact plus **its own dials**.
 *
 * ⚠ **Only the second half can go stale.** `palette.json` is a build output and `cargo xtask check`
 * fails when it is stale; a project's dials change every time someone saves. So this **rebuilds on
 * save and replaces** — appending would leave a deleted dial on the palette, offering a node whose
 * read has nothing behind it.
 */
export function mergedPalette(dials: Dial[]): PaletteNode[] {
  return [...palette(), ...dials.map(dialNode)];
}

/**
 * The read node for one dial — `<owner path>.<name>#dial`.
 *
 * ⚠ **Pure, and carrying the dial's real type.** A wrapper type would make every consumer unwrap, and
 * the pin type is what rejects a bad connection — wrapping it would disable the check that makes the
 * whole connection rule worth having.
 */
export function dialNode(dial: Dial): PaletteNode {
  const [owner = "", name = dial.id] = dial.id.includes(".")
    ? [dial.id.slice(0, dial.id.lastIndexOf(".")), dial.id.slice(dial.id.lastIndexOf(".") + 1)]
    : ["", dial.id];
  return {
    op: `${owner}.${name}#dial`,
    label: `dial ${name}`,
    category: owner || "Dials",
    doc: dial.doc ?? `The \`${dial.id}\` dial.`,
    outputs: [{ name: "value", type: dialType(dial) }],
  };
}

/** A dial's pin type — its real one, never a wrapper. */
function dialType(dial: Dial): string {
  switch (dial.kind.toLowerCase()) {
    case "number":
    case "range":
    case "adaptive":
    case "curve":
    case "table":
      return "float";
    case "bool":
      return "bool";
    case "enum":
      return "String";
    default:
      return "float";
  }
}

// ---------------------------------------------------------------------------------------------
// P04 — connection rules
// ---------------------------------------------------------------------------------------------

/** One end of a wire. */
export interface Pin {
  name: string;
  type: string;
  dir: "in" | "out";
}

/** Why a wire will not draw. */
export type Refusal = { ok: true } | { ok: false; why: string };

const GENERIC = /^(Ref|Kind|Asset)<(.+)>$/;

/** Split `Ref<Actor>` into `["Ref", "Actor"]`. */
function generic(type: string): [string, string] | undefined {
  const m = GENERIC.exec(type);
  return m && m[1] && m[2] ? [m[1], m[2]] : undefined;
}

/** Is `sub` the same class as `base`, or below it? */
function isOrDescends(sub: string, base: string): boolean {
  if (sub === base) return true;
  const found: ClassDef | undefined = classAt(sub) ?? classAt(`/Core/${sub}`);
  if (!found) return false;
  return found.ancestry.some((a) => a === base || a === `/Core/${base}` || a.endsWith(`/${base}`));
}

/**
 * May a wire be drawn from `from` to `to`?
 *
 * ⚠ **Refusals carry a reason even though the wire simply does not draw.** The reason is what a
 * tooltip shows and what a test asserts on; a silent `false` is indistinguishable from a bug in the
 * drag handling.
 */
export function mayConnect(from: Pin, to: Pin): Refusal {
  if (from.dir !== "out" || to.dir !== "in") {
    return { ok: false, why: "a wire runs from an output pin to an input pin" };
  }

  const exec = (t: string) => t === "exec";
  if (exec(from.type) !== exec(to.type)) {
    return { ok: false, why: "execution and data pins do not connect" };
  }
  if (exec(from.type)) return { ok: true };

  if (from.type === to.type) return { ok: true };

  const a = generic(from.type);
  const b = generic(to.type);

  // ⚠ **`Kind<T>` never connects to `Ref<T>`.** A class is not an instance, and the whole authoring
  // model rests on the distinction — this is the example the design names when it defines *impossible*.
  if (a && b && a[0] !== b[0]) {
    return {
      ok: false,
      why: `\`${from.type}\` is a ${a[0] === "Kind" ? "class" : "value"} and \`${to.type}\` wants a ${
        b[0] === "Kind" ? "class" : "value"
      } — they are different things, not different spellings`,
    };
  }

  // Same wrapper: a derived reference satisfies a base one, never the reverse.
  if (a && b && a[0] === b[0]) {
    if (isOrDescends(a[1], b[1])) return { ok: true };
    return {
      ok: false,
      why: `\`${a[1]}\` is not a \`${b[1]}\`${
        isOrDescends(b[1], a[1]) ? " — the wire runs the wrong way; a base does not satisfy a derived pin" : ""
      }`,
    };
  }

  // ⚠ **No implicit numeric conversion.** The compiler does not type-check, so a conversion permitted
  // here reaches lowering unchecked — and a graph that computes on a value the VM never converted is
  // wrong in a way no finding reports. An explicit node is one click and cannot be silent.
  return {
    ok: false,
    why: `\`${from.type}\` does not connect to \`${to.type}\`${
      NUMERIC.has(from.type) && NUMERIC.has(to.type) ? " — insert a conversion node rather than relying on one" : ""
    }`,
  };
}

const NUMERIC = new Set(["int", "float"]);

// ---------------------------------------------------------------------------------------------
// P04a — the widget follows from the type
// ---------------------------------------------------------------------------------------------

/** What a pin's editor looks like. */
export type Widget =
  | { kind: "number" }
  | { kind: "toggle" }
  | { kind: "text" }
  | { kind: "vector"; components: number }
  | { kind: "enum"; path: string }
  | { kind: "class-picker"; of: string }
  | { kind: "asset-then-row"; of: string }
  | { kind: "wire-only" };

/**
 * The widget for a pin type.
 *
 * ⚠ **A consequence of the type, never a per-pin choice.** If two pins of the same type could be given
 * different widgets, then the widget carries information the type does not — and the next person to add
 * a pin of that type has to know a convention nobody wrote down.
 */
export function widgetFor(type: string): Widget {
  if (type === "exec") return { kind: "wire-only" };
  if (type === "bool") return { kind: "toggle" };
  if (type === "int" || type === "float") return { kind: "number" };
  if (type === "String") return { kind: "text" };
  if (type === "Vec2") return { kind: "vector", components: 2 };
  if (type === "Vec3") return { kind: "vector", components: 3 };

  const g = generic(type);
  if (g?.[0] === "Kind") return { kind: "class-picker", of: g[1] };
  // ⚠ An `Unlock` picks **asset then row**: a table is a file, and a row inside it is not addressable
  // until the file is chosen. One combined picker would have to offer every row of every table.
  if (g?.[0] === "Asset" || g?.[1] === "Unlock" || type === "Unlock") {
    return { kind: "asset-then-row", of: g?.[1] ?? "Unlock" };
  }
  if (g?.[0] === "Ref") return { kind: "wire-only" };
  if (classAt(`/Core/${type}`)?.kind === "enum") return { kind: "enum", path: `/Core/${type}` };
  return { kind: "wire-only" };
}

// ---------------------------------------------------------------------------------------------
// P05a — Expression
// ---------------------------------------------------------------------------------------------

/** Why an expression was refused. */
export class ExpressionError extends Error {}

/**
 * Check an `Expression` node's text.
 *
 * > ⚠ **If it has a name in the API, it is a node. If it is an operator, it can be text.**
 *
 * Member access is forbidden deliberately, and the third reason is the good one: feeding `v1.x` into a
 * pin named `x1` is more descriptive, keeps the graph showing which components are involved, **and
 * lets the pin type reject bad input** — an error caught at the wire rather than inside a string.
 */
export function checkExpression(text: string): void {
  const t = text.trim();
  if (t.length === 0) throw new ExpressionError("an expression needs a formula");

  // ⚠ Method calls first: `origin.distance_to(target)` is a member access *and* a call, and naming it
  // as a call is the more useful message.
  if (/[A-Za-z_]\s*\(/.test(t.replace(/\b(if|then|else)\b/g, ""))) {
    throw new ExpressionError(
      `\`${t}\` calls something — that is a method node. If it has a name in the API, it is a node`,
    );
  }
  if (/\b\w+\s*\.\s*\w+/.test(t)) {
    throw new ExpressionError(
      `\`${t}\` reaches into a value — break the struct and feed a named pin instead, so the graph ` +
        `shows which components are involved and the pin type can reject bad input`,
    );
  }
  if (/\b(if|then|else)\b/.test(t)) {
    throw new ExpressionError(`\`${t}\` branches — that is a Branch node`);
  }
  if (!/^[\w\s+\-*/%().,]+$/.test(t)) {
    throw new ExpressionError(`\`${t}\` uses something that is not arithmetic`);
  }
}
