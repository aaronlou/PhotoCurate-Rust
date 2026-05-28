#!/usr/bin/env node
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("..", import.meta.url));
const pkg = JSON.parse(readFileSync(join(repoRoot, "package.json"), "utf8"));
const version = pkg.version;
const repo = process.env.GITHUB_REPOSITORY ?? "aaronlou/PhotoCurate-Rust";
const tag = process.env.RELEASE_TAG ?? `v${version}`;
const notes = process.env.RELEASE_NOTES ?? `PhotoCurate ${version}`;
const pubDate = process.env.RELEASE_PUB_DATE ?? new Date().toISOString();
const bundleDir = join(repoRoot, "src-tauri", "target", "release", "bundle");

const platforms = [
  {
    target: "darwin-aarch64",
    file: join(bundleDir, "macos", `PhotoCurate.app.tar.gz`),
  },
  {
    target: "darwin-aarch64-app",
    file: join(bundleDir, "macos", `PhotoCurate.app.tar.gz`),
  },
];

const manifestPlatforms = {};

for (const platform of platforms) {
  if (!existsSync(platform.file)) {
    continue;
  }
  const signaturePath = `${platform.file}.sig`;
  if (!existsSync(signaturePath)) {
    throw new Error(`Missing updater signature: ${signaturePath}`);
  }

  const filename = basename(platform.file);
  manifestPlatforms[platform.target] = {
    signature: readFileSync(signaturePath, "utf8").trim(),
    url: `https://github.com/${repo}/releases/download/${tag}/${filename}`,
  };
}

if (Object.keys(manifestPlatforms).length === 0) {
  throw new Error(
    "No updater artifacts found. Run `npm run release:macos` to build and sign updater assets."
  );
}

const manifest = {
  version,
  notes,
  pub_date: pubDate,
  platforms: manifestPlatforms,
};

const outputPath = process.argv[2] ?? join(bundleDir, "latest.json");
writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);

const digest = createHash("sha256").update(readFileSync(outputPath)).digest("hex");
console.log(`Wrote ${outputPath}`);
console.log(`sha256:${digest}`);
