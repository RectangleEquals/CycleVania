/**
 * DEV-ONLY golden-vector generator (not part of the published package, not run
 * by tests). It imports the REFERENCE determinism math (the upstream CrawlStar
 * `Shared/src/math/{rng,trig}.ts`) AND this package's ported copies, runs an
 * identical fixed draw-script (from `src/math/golden/record.ts`) through both,
 * asserts they agree bit-for-bit, and writes the agreed values to
 * `src/math/golden/*.json`.
 *
 * The committed JSON is what the hermetic `golden.test.ts` checks against, so the
 * test suite never reaches outside this package. Re-run this only when the
 * reference math legitimately changes:  `node scripts/gen-golden.ts`
 */
import { writeFileSync, mkdirSync } from "node:fs";
import { pathToFileURL, fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { recordRng, recordTrig, type RngModuleLike, type TrigModuleLike } from "../src/math/golden/record.ts";

const here = dirname(fileURLToPath(import.meta.url));
const goldenDir = resolve(here, "../src/math/golden");

// Reference (upstream) source of truth vs. this package's ported copies.
const REF_RNG = "E:/Projects/JS/Crawl/Public/Shared/src/math/rng.ts";
const REF_TRIG = "E:/Projects/JS/Crawl/Public/Shared/src/math/trig.ts";
const PORT_RNG = resolve(here, "../src/math/rng.ts");
const PORT_TRIG = resolve(here, "../src/math/trig.ts");

const load = async <T>(p: string): Promise<T> => (await import(pathToFileURL(p).href)) as T;

function assertEqual(name: string, ref: unknown, port: unknown): void {
  const a = JSON.stringify(ref);
  const b = JSON.stringify(port);
  if (a !== b) {
    console.error(`PARITY FAILURE in ${name}:\n  ref =${a}\n  port=${b}`);
    process.exit(1);
  }
}

const [refRng, portRng, refTrig, portTrig] = await Promise.all([
  load<RngModuleLike>(REF_RNG),
  load<RngModuleLike>(PORT_RNG),
  load<TrigModuleLike>(REF_TRIG),
  load<TrigModuleLike>(PORT_TRIG),
]);

const rngRef = recordRng(refRng);
assertEqual("rng", rngRef, recordRng(portRng));

const trigRef = recordTrig(refTrig);
assertEqual("trig", trigRef, recordTrig(portTrig));

mkdirSync(goldenDir, { recursive: true });
writeFileSync(resolve(goldenDir, "rng.golden.json"), JSON.stringify(rngRef, null, 2) + "\n");
writeFileSync(resolve(goldenDir, "trig.golden.json"), JSON.stringify(trigRef, null, 2) + "\n");
console.log("golden vectors written + reference↔port parity verified ✓");
