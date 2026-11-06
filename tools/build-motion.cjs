const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");

const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "casefile-motion-"));
const output = path.resolve(__dirname, "../ui/vendor");
try {
  execFileSync(process.platform === "win32" ? "npm.cmd" : "npm", ["install", "--prefix", temporary, "motion@14.2.0", "cobe@0.6.4", "esbuild@0.25.12"], { stdio: "inherit" });
  const entry = path.join(temporary, "entry.js");
  fs.writeFileSync(entry, 'export { animate, animateLayout } from "motion";\n');
  fs.mkdirSync(output, { recursive: true });
  require(path.join(temporary, "node_modules/esbuild")).buildSync({ entryPoints: [entry], bundle: true, format: "esm", minify: true, outfile: path.join(output, "motion.js") });
  const globeEntry = path.join(temporary, "cobe-entry.js");
  fs.writeFileSync(globeEntry, 'export { default } from "cobe";\n');
  require(path.join(temporary, "node_modules/esbuild")).buildSync({ entryPoints: [globeEntry], bundle: true, format: "esm", minify: true, outfile: path.join(output, "cobe.js") });
  const licenses = ["motion", "framer-motion", "motion-dom", "motion-utils", "cobe", "phenomenon"].map(name => {
    const root = path.join(temporary, "node_modules", name);
    const file = ["LICENSE.md", "LICENSE"].map(f => path.join(root, f)).find(fs.existsSync);
    if (!file) throw new Error("Missing license: " + name);
    return name + "\n" + fs.readFileSync(file, "utf8");
  });
  fs.writeFileSync(path.join(output, "LICENSES.txt"), licenses.join("\n\n"));
} finally {
  fs.rmSync(temporary, { recursive: true, force: true });
}
