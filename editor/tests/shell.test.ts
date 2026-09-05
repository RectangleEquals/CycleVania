/**
 * **The theme, the pins, and the Content panel.**
 *
 * ⚠ **The assertions are about what must stay *derived* and what must stay *distinguishable*.** A theme
 * renders whatever you give it; the claim worth checking is that nothing picks a hex of its own, because
 * that is how a palette rots one addition at a time.
 */

import { describe, expect, it } from "vitest";
import { DEFAULT, cssVariables, step, surfaces, type ThemeParams } from "../src/theme.ts";
import { EXEC, looksDifferent, nodeColour, pinColour } from "../src/pins.ts";
import { shellStyles } from "../src/shell.ts";

describe("P01 — the theme is four parameters, everything derived", () => {
  it("moves every surface when the ground moves", () => {
    // ⚠ **A step is a relationship, not a colour.** A declared panel colour beside a changed background
    // is a contrast bug nobody notices until a screenshot.
    const a = surfaces(DEFAULT);
    const b = surfaces({ ...DEFAULT, base: "#101418" });
    expect(b.panel).not.toBe(a.panel);
    expect(b.line).not.toBe(a.line);
    expect(b.text).not.toBe(a.text);
  });

  it("separates the steps, so a dense tree is readable", () => {
    const s = surfaces(DEFAULT);
    expect(new Set([s.bg, s.panel, s.raised, s.line]).size).toBe(4);
  });

  it("widens the steps when contrast rises", () => {
    const low = step(DEFAULT.base, 4, 0.5);
    const high = step(DEFAULT.base, 4, 2);
    expect(low).not.toBe(high);
  });

  it("has one accent, and selection derives from it", () => {
    // ⚠ One accent, so *"this is the live thing"* has exactly one answer.
    const s = surfaces({ ...DEFAULT, accent: "#ff8800" });
    expect(s.accent).toBe("#ff8800");
    expect(s.selected.startsWith("#ff8800")).toBe(true);
  });

  it("has exactly three semantic colours", () => {
    // ⚠ A fourth is a request to invent a meaning nobody defined.
    const s = surfaces();
    expect([s.ok, s.warn, s.err].filter(Boolean)).toHaveLength(3);
  });

  it("publishes every surface as a custom property", () => {
    const css = cssVariables();
    for (const key of Object.keys(surfaces())) expect(css).toContain(`--cv-${key}:`);
  });

  it("is the only place the stylesheet gets colour from", () => {
    // ⚠ **The claim this milestone rests on.** A component computing its own colour is a second theme,
    // and the second one goes stale the first time the first one moves.
    const bare = shellStyles().replace(/var\(--cv-[a-z]+\)/g, "");
    expect(bare).not.toMatch(/#[0-9a-fA-F]{3,6}/);
  });
});

describe("P03 — pin colour by type, exec white", () => {
  it("makes exec unmistakable, and nothing else that colour", () => {
    expect(pinColour("exec")).toBe(EXEC);
    for (const t of ["bool", "int", "float", "String", "Ref<Actor>", "Kind<Actor>"]) {
      expect(pinColour(t), t).not.toBe(EXEC);
    }
  });

  it("distinguishes the pair that will never connect", () => {
    // ⚠ **The payoff.** `Kind<T>` and `Ref<T>` cannot meet, so they must not look alike — colour is
    // what predicts the refusal before a developer tries the wire.
    expect(looksDifferent("Kind<Actor>", "Ref<Actor>")).toBe(true);
  });

  it("gives the pair that *does* connect the same colour", () => {
    // ⚠ **Coloured by the wrapper, not the parameter.** `Ref<Item>` satisfies `Ref<Actor>`; colouring
    // by `T` would make the two that connect look different and the two that cannot look alike —
    // exactly backwards.
    expect(pinColour("Ref<Item>")).toBe(pinColour("Ref<Actor>"));
  });

  it("keeps the primitives apart", () => {
    const seen = ["bool", "int", "float", "String"].map(pinColour);
    expect(new Set(seen).size).toBe(4);
  });

  it("colours a node's header by category", () => {
    // ⚠ Unreal's coding: a developer scanning a dense graph reads headers, not labels.
    const branch = nodeColour({ op: "core.branch" });
    const call = nodeColour({ op: "/Core/Actor.footprint" });
    const dial = nodeColour({ op: "/Content/Items/Hookshot.rope_length#dial" });
    expect(new Set([branch, call, dial]).size).toBe(3);
  });
});

describe("P04 — states, not decoration", () => {
  it("answers hover, selection and focus", () => {
    // ⚠ A control that does not answer the pointer reads as disabled.
    const css = shellStyles();
    for (const state of [":hover", ".is-selected", ":focus-visible", ":disabled"]) {
      expect(css, state).toContain(state);
    }
  });

  it("marks selection with more than colour", () => {
    // ⚠ Colour alone fails a reader who cannot separate two of them; the selected row also carries a
    // bar, so the state survives that.
    expect(shellStyles()).toMatch(/\.cv-row\.is-selected\s*\{[^}]*box-shadow/);
  });
});
