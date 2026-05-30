#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "dev" ]]; then
  shift
  exec bash scripts/tauri-dev.sh "$@"
fi

exec tauri "$@"
