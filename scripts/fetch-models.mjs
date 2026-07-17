// Download the local ML model weights that are too large to commit to git.
// Run automatically in CI before building, and locally via `pnpm fetch-models`.
// Skips any file that already exists.

import { createWriteStream, existsSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";

const MODELS = [
  {
    path: "src-tauri/models/ggml-base.en.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
  },
];

for (const model of MODELS) {
  if (existsSync(model.path)) {
    console.log(`✓ ${model.path} already present`);
    continue;
  }
  mkdirSync(dirname(model.path), { recursive: true });
  console.log(`↓ fetching ${model.url}`);
  const res = await fetch(model.url);
  if (!res.ok || !res.body) {
    console.error(`✗ download failed (${res.status}) for ${model.url}`);
    process.exit(1);
  }
  await pipeline(Readable.fromWeb(res.body), createWriteStream(model.path));
  console.log(`✓ saved ${model.path}`);
}

console.log("All models ready.");
