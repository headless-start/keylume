// The licences of everything a Keylume build ships: every crate compiled into the app for the
// target being built, and the npm packages bundled into its window. Writes
// src-tauri/THIRD-PARTY-NOTICES.txt (generated, not committed), which the Windows installer puts
// next to the app (src-tauri/windows/installer-hooks.nsh) and the release workflow publishes.
//
//   node tools/third_party_notices.mjs            # for this machine's target
//   TAURI_ENV_TARGET_TRIPLE=x86_64-pc-windows-msvc node tools/third_party_notices.mjs
//
// `tauri build` runs it (tauri.conf.json: beforeBuildCommand) with the target it builds. It reads
// only what's already on disk: Cargo's downloaded sources and node_modules. Works the same on
// Linux, macOS and Windows.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const OUT = "src-tauri/THIRD-PARTY-NOTICES.txt";
const LICENCE_FILE = /^(licen[cs]e|copying|notice|copyright|unlicense)([-._].*)?$/i;

const target = process.env.TAURI_ENV_TARGET_TRIPLE || execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/host: (\S+)/)[1];
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8", maxBuffer: 512 << 20 });

/** The licence texts shipped in a package's folder, in a stable order. */
function texts(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((f) => LICENCE_FILE.test(f) && statSync(join(dir, f)).isFile())
    .sort()
    .map((f) => readFileSync(join(dir, f), "utf8").replace(/\r\n/g, "\n").trim());
}

// ---- crates: everything the app links, through normal (not build or dev) dependencies ----
const meta = JSON.parse(run("cargo", ["metadata", "--format-version", "1", "--locked", "--filter-platform", target]));
const byId = new Map(meta.packages.map((p) => [p.id, p]));
const nodes = new Map(meta.resolve.nodes.map((n) => [n.id, n]));
const app = meta.packages.find((p) => p.name === "keylume-app");
if (!app) throw new Error("keylume-app isn't in the workspace");
const reached = new Set();
for (const stack = [app.id]; stack.length; ) {
  const id = stack.pop();
  if (reached.has(id)) continue;
  reached.add(id);
  for (const d of nodes.get(id)?.deps ?? []) if (d.dep_kinds.some((k) => k.kind === null)) stack.push(d.pkg);
}
const crates = [...reached]
  .map((id) => byId.get(id))
  .filter((p) => p.source) // the workspace's own crates are Keylume, under its own licence
  .map((p) => ({ name: p.name, version: p.version, licence: p.license ?? p.license_file ?? "unknown", home: p.repository ?? "", texts: texts(dirname(p.manifest_path)) }));

// ---- npm: the window's runtime dependencies ------------------------------------------------
// Walked in node_modules the way Node resolves them (not with `npm ls`: on Windows npm is a
// .cmd script that can't be run without a shell).
const readPkg = (dir) => JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
/** Where `name` resolves from package folder `from`: its own node_modules, then its parents'. */
function resolve(name, from) {
  for (let dir = from; ; dir = dirname(dir)) {
    const candidate = join(dir, "node_modules", name);
    if (existsSync(join(candidate, "package.json"))) return candidate;
    if (dirname(dir) === dir) return null;
  }
}
const found = new Map();
for (const stack = [[process.cwd(), readPkg(process.cwd())]]; stack.length; ) {
  const [dir, pkg] = stack.pop();
  for (const name of Object.keys(pkg.dependencies ?? {})) {
    const at = resolve(name, dir);
    if (!at || found.has(at)) continue;
    const dep = readPkg(at);
    found.set(at, dep);
    stack.push([at, dep]);
  }
}
const packages = [...found].map(([dir, pkg]) => {
  const licence = typeof pkg.license === "string" ? pkg.license : pkg.license?.type ?? "unknown";
  const home = typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url ?? "";
  return { name: pkg.name, version: pkg.version, licence, home, texts: texts(dir) };
});

// ---- one section per distinct licence text, listing everything under it --------------------
const all = [...crates, ...packages].sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
const groups = new Map();
for (const p of all) {
  const key = p.texts.length ? createHash("sha256").update(p.texts.join("\n\n")).digest("hex") : `spdx:${p.licence}`;
  if (!groups.has(key)) groups.set(key, { texts: p.texts, licence: p.licence, members: [] });
  groups.get(key).members.push(p);
}
const version = JSON.parse(readFileSync("package.json", "utf8")).version;
const rule = "-".repeat(78);
let out = `Licences for Keylume ${version} (${target})\n${"=".repeat(78)}\n\n`;
// Keylume's own licence travels with the app here, not on any screen of it
out += readFileSync("LICENSE", "utf8").replace(/\r\n/g, "\n").trim() + "\n\n" + "=".repeat(78) + "\n\n";
out += "Keylume is built ";
out += `with the ${crates.length} Rust crates and ${packages.length} npm packages below, each under its own licence,\n`;
out += "whose notices follow, grouped by licence text.\n";
for (const g of groups.values()) {
  out += `\n${rule}\n`;
  for (const m of g.members) out += `${m.name} ${m.version} (${m.licence})${m.home ? `  ${m.home}` : ""}\n`;
  out += "\n";
  const which = g.members.length === 1 ? "this package; it is" : "these packages; they are";
  out += g.texts.length ? g.texts.join(`\n\n`) + "\n" : `No licence file is published with ${which} licensed under ${g.licence} (see https://spdx.org/licenses/).\n`;
}
writeFileSync(OUT, out);
console.log(`wrote ${OUT}: ${crates.length} crates, ${packages.length} npm packages, ${groups.size} licence texts (${(out.length / 1024).toFixed(0)} KB)`);
