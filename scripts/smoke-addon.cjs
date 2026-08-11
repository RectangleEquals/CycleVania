// M00 smoke test: build the napi addon (`cargo build -p cv-bindings`), then load it from Node and
// call version(). Proves the native binding round-trips. Cross-platform dylib name resolution.
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const dll =
  process.platform === "win32"
    ? "cv_bindings.dll"
    : process.platform === "darwin"
      ? "libcv_bindings.dylib"
      : "libcv_bindings.so";

const src = path.join(root, "target", "debug", dll);
const outDir = path.join(root, "build");
const dest = path.join(outDir, "cyclevania.node");

if (!fs.existsSync(src)) {
  console.error(`addon not built: ${src} is missing. Run \`cargo build -p cv-bindings\` first.`);
  process.exit(1);
}

fs.mkdirSync(outDir, { recursive: true });
fs.copyFileSync(src, dest);

const addon = require(dest);
const v = addon.version();
console.log("addon.version() =>", v);

if (typeof v !== "string" || v.length === 0) {
  console.error("FAIL: version() did not return a non-empty string");
  process.exit(1);
}
console.log("OK: napi addon loaded from Node and returned a version string");
