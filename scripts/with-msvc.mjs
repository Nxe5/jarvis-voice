// Windows-only: set LIBCLANG / MSVC include paths so bindgen can find vcruntime.h,
// then run the remaining argv as a command. On other platforms this is a no-op passthrough.
//
//   node scripts/with-msvc.mjs tauri build
//   pnpm tauri build   (via package.json "tauri" script)

import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

function pickMsvcInclude(roots) {
  for (const root of roots) {
    const msvcRoot = join(root, "VC", "Tools", "MSVC");
    if (!existsSync(msvcRoot)) continue;
    const versions = readdirSync(msvcRoot).sort().reverse();
    for (const ver of versions) {
      const include = join(msvcRoot, ver, "include");
      if (existsSync(join(include, "vcruntime.h"))) {
        return { instance: root, include };
      }
    }
  }
  return null;
}

if (process.platform === "win32") {
  const cargoBin = join(process.env.USERPROFILE ?? "", ".cargo", "bin");
  const extras = [
    cargoBin,
    "C:\\Program Files\\CMake\\bin",
    "C:\\Program Files\\LLVM\\bin",
  ].filter((p) => existsSync(p));
  process.env.PATH = [...extras, process.env.PATH ?? ""].join(";");

  if (!process.env.LIBCLANG_PATH && existsSync("C:\\Program Files\\LLVM\\bin")) {
    process.env.LIBCLANG_PATH = "C:\\Program Files\\LLVM\\bin";
  }

  if (!process.env.BINDGEN_EXTRA_CLANG_ARGS) {
    const picked = pickMsvcInclude([
      "C:\\Program Files (x86)\\Microsoft Visual Studio\\18\\BuildTools",
      "C:\\Program Files\\Microsoft Visual Studio\\18\\Community",
      "C:\\Program Files\\Microsoft Visual Studio\\2022\\Community",
      "C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools",
    ]);
    const winSdk = "C:\\Program Files (x86)\\Windows Kits\\10\\Include\\10.0.26100.0";
    if (picked && existsSync(winSdk)) {
      process.env.CMAKE_GENERATOR ??= "Visual Studio 18 2026";
      process.env.CMAKE_GENERATOR_INSTANCE ??= picked.instance;
      process.env.BINDGEN_EXTRA_CLANG_ARGS = [
        `-I"${picked.include}"`,
        `-I"${join(winSdk, "ucrt")}"`,
        `-I"${join(winSdk, "shared")}"`,
        `-I"${join(winSdk, "um")}"`,
        `-I"${join(winSdk, "winrt")}"`,
        "-fms-compatibility",
        "-fms-extensions",
      ].join(" ");
    }
  }
}

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("usage: node scripts/with-msvc.mjs <command> [args...]");
  process.exit(1);
}

const result = spawnSync(args[0], args.slice(1), {
  stdio: "inherit",
  shell: true,
  env: process.env,
});
process.exit(result.status ?? 1);
