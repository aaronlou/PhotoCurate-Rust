#!/usr/bin/env bash
set -euo pipefail

if [[ ! -f updater.key ]]; then
  echo "Missing updater.key. Generate one with: npm run tauri signer generate -- --write-keys updater.key --ci" >&2
  exit 1
fi

export TAURI_SIGNING_PRIVATE_KEY
TAURI_SIGNING_PRIVATE_KEY="$(cat updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

export APPLE_SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:-Developer ID Application: Aaron Lou (V63B559WYX)}"
export APPLE_PROVIDER_SHORT_NAME="${APPLE_PROVIDER_SHORT_NAME:-V63B559WYX}"

npm run tauri-build -- --bundles app,dmg
npm run updater:manifest

echo
echo "Release assets:"
find src-tauri/target/release/bundle -maxdepth 3 -type f \
  \( -name 'PhotoCurate_*.dmg' -o -name 'PhotoCurate.app.tar.gz' -o -name 'PhotoCurate.app.tar.gz.sig' -o -name 'latest.json' \) \
  -print | sort
