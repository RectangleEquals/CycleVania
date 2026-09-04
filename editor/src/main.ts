/**
 * The editor's browser half.
 *
 * ⚠ **Deliberately almost nothing.** M16 proves the seam — open a project, list it, save a change that
 * survives a reload. The views arrive at M17-M19, and the layout they sit in needs mockups first
 * (`10-editor.md` §10), so anything built here now would be built twice.
 */

interface VersionReply {
  version: string;
}

async function main(): Promise<void> {
  const app = document.querySelector("#app");
  if (!app) return;
  try {
    const res = await fetch("/api/version");
    const { version } = (await res.json()) as VersionReply;
    // ⚠ The core already names itself — `cyclevania 0.2.1 (core …, determinism …)` — so a
    // prefix here rendered "CycleVania cyclevania 0.2.1". Only a screenshot showed it; every
    // assertion was about the version *number*, and the number was right.
    app.textContent = `${version} — editor service reachable.`;
  } catch {
    app.textContent = "the editor service is not running — `npm run serve`";
  }
}

void main();
