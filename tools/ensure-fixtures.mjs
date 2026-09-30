// The browser preview's library (public/mock/library.json) is generated, not committed:
// it's 20 MB+. `npm run dev` and `npm test` run this first and build it when it's missing.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

const OUT = "public/mock/library.json";
if (!existsSync(OUT)) {
  console.log(`${OUT} is missing: generating it (cargo run -p keylume-profiles --example dump)…`);
  const r = spawnSync("cargo", ["run", "-q", "-p", "keylume-profiles", "--example", "dump", "--", "--full"], {
    encoding: "utf8",
    maxBuffer: 256 << 20,
  });
  if (r.status !== 0) {
    console.error(r.stderr || r.error?.message);
    console.error(`\nCouldn't build ${OUT}. It needs Rust: install it (https://rustup.rs) and run "npm run fixtures".`);
    process.exit(1);
  }
  mkdirSync(dirname(OUT), { recursive: true });
  writeFileSync(OUT, r.stdout);
  console.log(`wrote ${OUT} (${(r.stdout.length / 1e6).toFixed(1)} MB)`);
}
