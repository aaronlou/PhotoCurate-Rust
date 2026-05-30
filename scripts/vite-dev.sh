#!/usr/bin/env bash
set -euo pipefail

if node -e "const net=require('net'); const s=net.connect(1420, '127.0.0.1'); s.once('connect',()=>{s.end(); process.exit(0)}); s.once('error',()=>process.exit(1)); setTimeout(()=>{s.destroy(); process.exit(1)}, 500);" >/dev/null 2>&1; then
  echo "Vite dev server already appears to be running on port 1420; reusing it."
  exit 0
fi

exec npm run dev
