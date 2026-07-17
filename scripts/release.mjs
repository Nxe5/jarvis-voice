// Cut a release. Every run bumps the version (patch by default; pass `minor`
// or `major`), verifies the build + tests pass locally, then commits, tags, and
// pushes. Pushing the `v*` tag triggers the GitHub Actions release workflow,
// which rebuilds on macOS/Windows/Linux and publishes the release once all
// platform builds and tests succeed.
//
//   pnpm release            # 0.1.0 -> 0.1.1
//   pnpm release minor      # 0.1.1 -> 0.2.0
//   pnpm release major      # 0.2.0 -> 1.0.0
//
// Set SKIP_RELEASE_CHECKS=1 to skip the local build/test gate (not recommended).

import { readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";

const run = (cmd, opts = {}) =>
  execSync(cmd, { stdio: "inherit", ...opts });
const capture = (cmd) => execSync(cmd, { encoding: "utf8" }).trim();

// --- 0. Safety: on main, clean working tree ------------------------------
const branch = capture("git rev-parse --abbrev-ref HEAD");
if (branch !== "main") {
  console.error(`✗ releases must be cut from 'main' (currently on '${branch}').`);
  process.exit(1);
}
if (capture("git status --porcelain")) {
  console.error("✗ working tree is dirty — commit or stash changes first.");
  process.exit(1);
}

// --- 1. Compute next version ---------------------------------------------
const bump = process.argv[2] ?? "patch";
if (!["patch", "minor", "major"].includes(bump)) {
  console.error(`✗ unknown bump '${bump}' (use patch | minor | major).`);
  process.exit(1);
}
const pkgPath = "package.json";
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
const [maj, min, pat] = pkg.version.split(".").map(Number);
const next =
  bump === "major"
    ? `${maj + 1}.0.0`
    : bump === "minor"
      ? `${maj}.${min + 1}.0`
      : `${maj}.${min}.${pat + 1}`;
const tag = `v${next}`;

console.log(`\n▶ Releasing ${tag}  (was v${pkg.version})\n`);

// --- 2. Write the version into all three manifests -----------------------
pkg.version = next;
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

const confPath = "src-tauri/tauri.conf.json";
const conf = JSON.parse(readFileSync(confPath, "utf8"));
conf.version = next;
writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n");

const cargoPath = "src-tauri/Cargo.toml";
const cargo = readFileSync(cargoPath, "utf8").replace(
  /^version = "\d+\.\d+\.\d+"/m,
  `version = "${next}"`,
);
writeFileSync(cargoPath, cargo);

// Keep Cargo.lock in sync with the new package version.
run("cargo update -p jarvis --manifest-path src-tauri/Cargo.toml --offline || true");

// --- 3. Local gate: build + test BEFORE tagging --------------------------
if (!process.env.SKIP_RELEASE_CHECKS) {
  console.log("\n▶ Verifying build + tests before tagging…\n");
  run("pnpm install --frozen-lockfile");
  run("node scripts/fetch-models.mjs");
  run("pnpm build"); // tsc + vite
  run("cargo test --manifest-path src-tauri/Cargo.toml");
} else {
  console.log("⚠ SKIP_RELEASE_CHECKS set — skipping local gate.");
}

// --- 4. Commit, tag, push ------------------------------------------------
run(
  `git add ${pkgPath} ${confPath} ${cargoPath} src-tauri/Cargo.lock`,
);
run(`git commit -m "release: ${tag}"`);
run(`git tag -a ${tag} -m "Jarvis ${tag}"`);
run("git push origin main");
run(`git push origin ${tag}`);

console.log(`\n✓ Pushed ${tag}. GitHub Actions will build + publish the release.`);
console.log(
  "  Watch: https://github.com/Nxe5/jarvis-voice/actions\n",
);
