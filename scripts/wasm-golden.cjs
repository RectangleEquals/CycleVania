// The wasm32 half of the cross-target determinism check (M02).
//
// Loads the `wasm_probe` example compiled to wasm32-unknown-unknown, calls its exports to get the
// determinism blob out of linear memory, and byte-compares it against the SAME golden fixture the
// native Rust test (`crates/cv-determinism/tests/cross_target.rs`) uses. If native and wasm both match
// one committed file, the cross-machine guarantee holds.
//
// Build first:
//   cargo build -p cv-determinism --example wasm_probe --target wasm32-unknown-unknown
// Or just run `npm run verify:determinism`, which does both halves.

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const wasmPath = path.join(
  root,
  "target",
  "wasm32-unknown-unknown",
  "debug",
  "examples",
  "wasm_probe.wasm",
);
const fixturePath = path.join(root, "golden", "vectors", "m02_determinism_probe.bin");

function fail(msg) {
  console.error(`FAIL: ${msg}`);
  process.exit(1);
}

if (!fs.existsSync(wasmPath)) {
  fail(
    `wasm probe not built at ${wasmPath}\n` +
      `      run: cargo build -p cv-determinism --example wasm_probe --target wasm32-unknown-unknown`,
  );
}
if (!fs.existsSync(fixturePath)) {
  fail(`golden fixture missing at ${fixturePath} (run the native test with CV_BLESS=1 first)`);
}

// NB: `module` is a reserved binding in CommonJS — hence `wasmModule`.
const wasmModule = new WebAssembly.Module(fs.readFileSync(wasmPath));

// A plain cdylib should need no imports; if the toolchain ever adds some, surface it clearly rather
// than dying inside instantiate().
const imports = WebAssembly.Module.imports(wasmModule);
if (imports.length > 0) {
  console.warn(
    `note: wasm module declares ${imports.length} import(s): ` +
      imports.map((i) => `${i.module}.${i.name}`).join(", "),
  );
}

const instance = new WebAssembly.Instance(wasmModule, {});
const { probe_len, probe_ptr, memory } = instance.exports;

if (typeof probe_len !== "function" || typeof probe_ptr !== "function") {
  fail("wasm module does not export probe_len/probe_ptr");
}
if (!(memory instanceof WebAssembly.Memory)) {
  fail("wasm module does not export its linear memory");
}

const len = probe_len();
const ptr = probe_ptr();
const actual = Buffer.from(new Uint8Array(memory.buffer, ptr, len)); // copy out before any growth
const expected = fs.readFileSync(fixturePath);

console.log(`wasm probe: ${len} bytes at 0x${ptr.toString(16)}`);
console.log(`fixture   : ${expected.length} bytes (${path.relative(root, fixturePath)})`);

if (actual.length !== expected.length) {
  fail(`length mismatch — wasm produced ${actual.length} bytes, fixture has ${expected.length}`);
}

if (!actual.equals(expected)) {
  // Point at the first divergence and decode both sides as f64, which is what they almost always are.
  let i = 0;
  while (i < actual.length && actual[i] === expected[i]) i++;
  const start = Math.floor(i / 8) * 8;
  const a = actual.subarray(start, start + 8);
  const e = expected.subarray(start, start + 8);
  fail(
    `wasm output diverges from native at byte ${i} (value index ${start / 8})\n` +
      `      wasm   : ${a.toString("hex")}  (as f64: ${a.readDoubleLE(0)})\n` +
      `      native : ${e.toString("hex")}  (as f64: ${e.readDoubleLE(0)})\n` +
      `      This is a cross-target determinism regression — a platform transcendental or a\n` +
      `      target-dependent operation has crept into cv-determinism.`,
  );
}

console.log("OK: wasm32 output is byte-identical to the native golden fixture");
