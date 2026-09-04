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
    app.textContent = `CycleVania ${version} — editor service reachable.`;
  } catch {
    app.textContent = "the editor service is not running — `npm run serve`";
  }
}

void main();
