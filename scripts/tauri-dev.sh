#!/usr/bin/env bash
set -euo pipefail

prepend_env_path() {
  local variable_name="$1"
  local path_value="$2"
  local existing_value="${!variable_name:-}"

  if [[ -n "${existing_value}" ]]; then
    export "${variable_name}=${path_value}:${existing_value}"
  else
    export "${variable_name}=${path_value}"
  fi
}

# StoreKit/IAP pulls Swift libraries into the dev binary. `cargo run` does not
# launch an app bundle, so dyld may miss Swift concurrency from the system cache.
# `/usr/lib/swift` resolves via dyld even when the individual file is not visible.
prepend_env_path "DYLD_LIBRARY_PATH" "/usr/lib/swift"
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,/usr/lib/swift"

swift_runtime_dir=""
if command -v xcrun >/dev/null 2>&1; then
  developer_dir="$(xcrun --show-sdk-platform-path 2>/dev/null || true)"
  toolchain_dir="$(xcrun --find swift 2>/dev/null || true)"
  toolchain_dir="${toolchain_dir%/usr/bin/swift}"

  candidates=(
    "${toolchain_dir}/usr/lib/swift/macosx"
    "${toolchain_dir}/usr/lib/swift-5.5/macosx"
    "${developer_dir}/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx"
    "${developer_dir}/Developer/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx"
  )

  for candidate in "${candidates[@]}"; do
    if [[ -f "${candidate}/libswift_Concurrency.dylib" ]]; then
      swift_runtime_dir="${candidate}"
      break
    fi
  done
fi

if [[ -n "${swift_runtime_dir}" ]]; then
  prepend_env_path "DYLD_FALLBACK_LIBRARY_PATH" "${swift_runtime_dir}"
fi

exec tauri dev "$@"
