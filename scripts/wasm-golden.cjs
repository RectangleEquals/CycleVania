// The wasm32 half of the cross-target determinism check.
//
// For each registered probe: load the crate's probe example compiled to wasm32-unknown-unknown, read
// its determinism blob out of linear memory, and byte-compare it against the SAME golden fixture the
// native Rust test uses. Native and wasm both matching one committed file is the cross-machine
// guarantee; either half alone proves nothing about portability.
//
// Build first (or use `npm run verify:determinism`, which does both halves):
//   cargo build -p cv-determinism --example determinism_probe --target wasm32-unknown-unknown
//   cargo build -p cv-core        --example core_probe        --target wasm32-unknown-unknown

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const WASM_DIR = path.join(root, "target", "wasm32-unknown-unknown", "debug", "examples");

/**
 * Every crate that carries a determinism probe. Add a row when a new crate gains one.
 * `example` names must stay unique across the workspace — all examples share one output directory.
 */
const PROBES = [
  {
    crate: "cv-determinism",
    example: "determinism_probe",
    fixture: "determinism_probe.bin",
    covers: "owned math, RNG streams, geometry + Mat4 kernels",
  },
  {
    crate: "cv-core",
    example: "core_probe",
    fixture: "core_probe.bin",
    covers: "arena layout, object identity, serialization, scope graph, structure math",
  },
];

/** Read a probe blob out of a compiled wasm module. */
function readProbe(wasmFile) {
  // NB: `module` is a reserved binding in CommonJS — hence `wasmModule`.
  const wasmModule = new WebAssembly.Module(fs.readFileSync(wasmFile));

  const imports = WebAssembly.Module.imports(wasmModule);
  if (imports.length > 0) {
    console.warn(
      `  note: module declares ${imports.length} import(s): ` +
        imports.map((i) => `${i.module}.${i.name}`).join(", "),
    );
  }

  const instance = new WebAssembly.Instance(wasmModule, {});
  const { probe_len, probe_ptr, memory } = instance.exports;

  if (typeof probe_len !== "function" || typeof probe_ptr !== "function") {
    throw new Error("module does not export probe_len/probe_ptr");
  }
  if (!(memory instanceof WebAssembly.Memory)) {
    throw new Error("module does not export its linear memory");
  }

  const len = probe_len();
  const ptr = probe_ptr();
  // Copy out before anything can grow memory and invalidate the view.
  return { blob: Buffer.from(new Uint8Array(memory.buffer, ptr, len)), ptr, len };
}

/** Report the first divergence pointing at the offending value, not just a byte offset. */
function describeMismatch(actual, expected) {
  let i = 0;
  while (i < actual.length && i < expected.length && actual[i] === expected[i]) i++;
  const start = Math.floor(i / 8) * 8;
  const a = actual.subarray(start, start + 8);
  const e = expected.subarray(start, start + 8);
  const asF64 = (b) => (b.length === 8 ? b.readDoubleLE(0) : "n/a");
  return (
    `diverges at byte ${i} (8-byte value index ${start / 8})\n` +
    `        wasm   : ${a.toString("hex")}  (as f64: ${asF64(a)})\n` +
    `        native : ${e.toString("hex")}  (as f64: ${asF64(e)})\n` +
    `        A cross-target determinism regression: a platform transcendental, a usize/isize in\n` +
    `        the serialized form, or another target-dependent operation has crept in.`
  );
}

let failed = 0;

for (const probe of PROBES) {
  const wasmFile = path.join(WASM_DIR, `${probe.example}.wasm`);
  const fixturePath = path.join(root, "golden", "vectors", probe.fixture);

  console.log(`\n${probe.crate} — ${probe.covers}`);

  if (!fs.existsSync(wasmFile)) {
    console.error(
      `  FAIL: probe not built at ${path.relative(root, wasmFile)}\n` +
        `        run: cargo build -p ${probe.crate} --example ${probe.example} ` +
        `--target wasm32-unknown-unknown`,
    );
    failed++;
    continue;
  }
  if (!fs.existsSync(fixturePath)) {
    console.error(`  FAIL: fixture missing at ${path.relative(root, fixturePath)} (bless natively first)`);
    failed++;
    continue;
  }

  let result;
  try {
    result = readProbe(wasmFile);
  } catch (err) {
    console.error(`  FAIL: ${err.message}`);
    failed++;
    continue;
  }

  const expected = fs.readFileSync(fixturePath);
  console.log(
    `  wasm ${result.len} bytes @ 0x${result.ptr.toString(16)} vs fixture ${expected.length} bytes`,
  );

  if (result.blob.length !== expected.length) {
    console.error(`  FAIL: length mismatch — wasm ${result.blob.length}, fixture ${expected.length}`);
    failed++;
  } else if (!result.blob.equals(expected)) {
    console.error(`  FAIL: ${describeMismatch(result.blob, expected)}`);
    failed++;
  } else {
    console.log("  OK: byte-identical to the native golden fixture");
  }
}

console.log("");
if (failed > 0) {
  console.error(`${failed} of ${PROBES.length} probe(s) FAILED`);
  process.exit(1);
}
console.log(`All ${PROBES.length} probes byte-identical across native and wasm32.`);
