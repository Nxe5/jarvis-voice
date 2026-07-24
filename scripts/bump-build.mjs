// Bump the patch/build number across package.json, tauri.conf.json, and
// Cargo.toml before a local build. Skipped in CI / tagged releases so the
// version cut by `pnpm release` stays stable.
//
//   node scripts/bump-build.mjs
//   SKIP_BUILD_BUMP=1 node scripts/bump-build.mjs   # no-op

import { readFileSync, writeFileSync } from "node:fs";

if (process.env.SKIP_BUILD_BUMP === "1" || process.env.CI === "true") {
  console.log("bump-build: skipped (CI or SKIP_BUILD_BUMP)");
  process.exit(0);
}

const pkgPath = "package.json";
const confPath = "src-tauri/tauri.conf.json";
const cargoPath = "src-tauri/Cargo.toml";

const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
const parts = pkg.version.split(".").map(Number);
if (parts.length !== 3 || parts.some((n) => Number.isNaN(n))) {
  console.error(`bump-build: bad version '${pkg.version}' (expected x.y.z)`);
  process.exit(1);
}
const [maj, min, pat] = parts;
const next = `${maj}.${min}.${pat + 1}`;

pkg.version = next;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

const conf = JSON.parse(readFileSync(confPath, "utf8"));
conf.version = next;
writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n");

const cargo = readFileSync(cargoPath, "utf8").replace(
  /^version = "\d+\.\d+\.\d+"/m,
  `version = "${next}"`,
);
writeFileSync(cargoPath, cargo);

console.log(`bump-build: ${maj}.${min}.${pat} → ${next}`);
