/**
 * **The theme — four parameters, everything derived.**
 *
 * ⚠ **Not a palette somebody extends by picking a hex that looked right beside the last one.** That is
 * how a palette rots: the twentieth element is chosen against the nineteenth, nobody re-checks the
 * first, and the surface drifts apart one addition at a time.
 *
 * ▶ **Godot's model**, and the only thing this editor takes from Godot besides the properties panel —
 * its theme is generated from base, accent, contrast and corner radius rather than declared per widget.
 * The dark ground and the single accent are Unreal's.
 *
 * # Why derived rather than declared
 *
 * ⚠ **A step is a relationship, not a colour.** `panel` is *one step up from the ground*; if the ground
 * moves, the panel moves with it and stays legible. A declared `#171b22` beside a changed background is
 * a contrast bug nobody notices until a screenshot.
 */

/** What the whole surface is generated from. */
export interface ThemeParams {
  /** The chrome's ground. Everything else is a step from it. */
  base: string;
  /** ⚠ **One accent**, so *"this is the live thing"* has exactly one answer. */
  accent: string;
  /** How far the steps sit from base — what makes a dense tree readable. */
  contrast: number;
  /** The single roundness the whole surface shares, in px. */
  radius: number;
}

/** ⚠ Unreal's ground, not Godot's: darker, so a viewport is the bright thing on screen. */
export const DEFAULT: ThemeParams = {
  base: "#1a1d21",
  accent: "#3d8bfd",
  contrast: 1,
  radius: 4,
};

/** Parse `#rrggbb` into components. */
function rgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

function hex(r: number, g: number, b: number): string {
  const c = (n: number) =>
    Math.max(0, Math.min(255, Math.round(n)))
      .toString(16)
      .padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

/**
 * A step away from the ground.
 *
 * ⚠ **Steps are multiplicative, not additive.** Adding a fixed amount to a near-black ground produces
 * visible banding at the bottom and no separation at the top; scaling keeps the ratio a reader's eye
 * actually responds to.
 */
export function step(base: string, n: number, contrast: number): string {
  const [r, g, b] = rgb(base);
  const k = 1 + 0.16 * n * contrast;
  return hex(r * k, g * k, b * k);
}

/** Blend toward white, for text on a dark ground. */
function lift(base: string, amount: number): string {
  const [r, g, b] = rgb(base);
  return hex(r + (255 - r) * amount, g + (255 - g) * amount, b + (255 - b) * amount);
}

/** Every surface the editor draws on, derived from the four. */
export interface Surfaces {
  bg: string;
  panel: string;
  raised: string;
  line: string;
  text: string;
  muted: string;
  accent: string;
  /** ⚠ Selection is the accent at low alpha over the panel — not a fifth colour. */
  selected: string;
  radius: string;
  /** ⚠ **Three semantic colours, and no more.** A fourth is a request to invent a meaning nobody defined. */
  ok: string;
  warn: string;
  err: string;
}

/** Derive every surface from the parameters. */
export function surfaces(p: ThemeParams = DEFAULT): Surfaces {
  return {
    bg: p.base,
    panel: step(p.base, 1, p.contrast),
    raised: step(p.base, 2, p.contrast),
    line: step(p.base, 4, p.contrast),
    text: lift(p.base, 0.82),
    muted: lift(p.base, 0.48),
    accent: p.accent,
    selected: `${p.accent}33`,
    radius: `${p.radius}px`,
    ok: "#7bbf6a",
    warn: "#e0a33a",
    err: "#e06a5c",
  };
}

/** The theme as CSS custom properties, for anything that would rather write CSS than call a function. */
export function cssVariables(p: ThemeParams = DEFAULT): string {
  const s = surfaces(p);
  return Object.entries(s)
    .map(([k, v]) => `--cv-${k}: ${v};`)
    .join(" ");
}

/**
 * Install the theme on the document.
 *
 * ⚠ **On `:root`, so every surface reads the same values.** A component that computed its own colours
 * would be a second theme, and the second one is the one that goes stale.
 */
export function install(p: ThemeParams = DEFAULT): Surfaces {
  const s = surfaces(p);
  const root = document.documentElement;
  for (const [k, v] of Object.entries(s)) root.style.setProperty(`--cv-${k}`, v);
  document.body.style.background = s.bg;
  document.body.style.color = s.text;
  return s;
}
