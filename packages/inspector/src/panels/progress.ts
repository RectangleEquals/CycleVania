/**
 * Default progress overlay — renders the `GenProgress` stream plainly (overall bar,
 * phase label, elapsed). Self-contained so it doubles as the documented host example
 * for rendering CycleVania's async progress.
 */

import { store } from "../state.js";
import { el, clear } from "../dom.js";

export function renderProgress(host: HTMLElement): void {
  const { generating, progress } = store.state;
  host.className = generating ? "overlay show" : "overlay";
  clear(host);
  if (!generating) return;

  const card = el("div", { class: "card" });
  card.append(el("div", { style: "font-weight:bold; margin-bottom:10px" }, "Generating world…"));

  const track = el("div", { class: "progress-track" });
  const fill = el("div", { class: "progress-fill" });
  fill.style.width = `${Math.round((progress?.fraction ?? 0) * 100)}%`;
  track.append(fill);
  card.append(track);

  const pct = Math.round((progress?.fraction ?? 0) * 100);
  const secs = ((progress?.elapsedMs ?? 0) / 1000).toFixed(1);
  card.append(el("div", { style: "margin-top:10px; color:var(--muted)" }, progress ? `${progress.label} — ${pct}% · ${secs}s elapsed` : "starting…"));
  host.append(card);
}
