#!/usr/bin/env bash
# Build the ARM64 HarmonyOS library before invoking hvigor/CMake.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -f "$ROOT/scripts/dev.local.env" ]]; then source "$ROOT/scripts/dev.local.env"; fi
SDK="${PHOTOCRAFT_NATIVE_SDK:-${DEVECO_SDK_HOME:-${HOS_SDK_HOME:-/Applications/DevEco-Studio.app/Contents/sdk}}/default/openharmony/native}"
# CI SDK locations may already point at openharmony, or at its native directory.
if [[ ! -d "$SDK/llvm" ]]; then
  for candidate in "${DEVECO_SDK_HOME:-}/openharmony/native" "${DEVECO_SDK_HOME:-}/native" "${DEVECO_SDK_HOME:-}"; do
    if [[ -d "$candidate/llvm" && -d "$candidate/sysroot" ]]; then SDK="$candidate"; break; fi
  done
fi
[[ -x "$SDK/llvm/bin/clang" ]] || { echo "HarmonyOS native SDK not found: $SDK" >&2; exit 1; }
TARGETS="${PHOTOCRAFT_NATIVE_TARGETS:-aarch64-unknown-linux-ohos}"
PROFILE="${PHOTOCRAFT_RUST_PROFILE:-release}"
HAP_VERSION="${PHOTOCRAFT_HAP_VERSION:-$(python3 - "$ROOT/AppScope/app.json5" <<'PYVERSION'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as f:
    print(json.load(f)["app"]["versionName"])
PYVERSION
)}"
[[ "$HAP_VERSION" =~ ^[0-9]+(\.[0-9]+){1,3}$ ]] || { echo "Invalid HAP version: $HAP_VERSION" >&2; exit 2; }
ARGS=()
# Explicit dev profile also avoids expanding an empty array under macOS Bash 3's nounset.
case "$PROFILE" in release) ARGS+=(--release);; debug) ARGS+=(--profile dev);; *) echo 'Profile must be release or debug' >&2; exit 2;; esac
for target in $TARGETS; do
  case "$target" in aarch64-unknown-linux-ohos) ;; *) echo "Unsupported target: $target" >&2; exit 2;; esac
  [[ -d "$(rustc --print sysroot)/lib/rustlib/$target" ]] || { echo "Install target with: rustup target add $target" >&2; exit 1; }
  compiler="$SDK/llvm/bin/$target-clang"
  suffix="${target//-/_}"
  upper="$(printf '%s' "$suffix" | tr '[:lower:]' '[:upper:]')"
  env "CC_${suffix}=$compiler" "AR_${suffix}=$SDK/llvm/bin/llvm-ar" \
      "CARGO_TARGET_${upper}_LINKER=$compiler" \
      "PHOTOCRAFT_HAP_VERSION=$HAP_VERSION" \
      cargo build --locked --manifest-path "$ROOT/native/rust/Cargo.toml" --target "$target" "${ARGS[@]}"
done
# Cargo is finished: clear its host debug cache, retaining ARM64 build outputs.
# This also covers dev.sh, offline packaging and CI, which use this script.
python3 "$ROOT/scripts/clean_rust_debug.py" || echo 'Warning: host debug cache cleanup failed' >&2
