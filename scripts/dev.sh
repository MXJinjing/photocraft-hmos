#!/usr/bin/env bash
# Native development: rebuild Rust/C++/ArkTS and install, or launch an existing HAP.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -f "$ROOT/scripts/dev.local.env" ]]; then source "$ROOT/scripts/dev.local.env"; fi
HDC="${PHOTOCRAFT_HDC:-/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc}"
HVIGOR="${PHOTOCRAFT_HVIGOR:-/Applications/DevEco-Studio.app/Contents/tools/hvigor/bin/hvigorw}"
OHPM="${PHOTOCRAFT_OHPM:-/Applications/DevEco-Studio.app/Contents/tools/ohpm/bin/ohpm}"
export DEVECO_SDK_HOME="${DEVECO_SDK_HOME:-/Applications/DevEco-Studio.app/Contents/sdk}"
export JAVA_HOME="${JAVA_HOME:-/Applications/DevEco-Studio.app/Contents/jbr/Contents/Home}"
export PATH="/Applications/DevEco-Studio.app/Contents/tools/node/bin:$PATH"
BUNDLE=io.github.storytold.photocraft.hmos
one_device() {
  count="$("$HDC" list targets | tr -d '\r' | awk 'NF && $0 != "[Empty]" {n++} END {print n+0}')"
  [[ "$count" == 1 ]] || { echo "Connect exactly one simulator/device ($count found)" >&2; exit 1; }
}
launch() { one_device; "$HDC" shell aa start -a EntryAbility -b "$BUNDLE"; }
case "${1:-run}" in
  build|run)
    "$ROOT/scripts/build_native.sh"
    cd "$ROOT"
    "$OHPM" install
    "$HVIGOR" assembleHap --mode module -p product=default -p buildMode=debug --no-daemon
    python3 "$ROOT/scripts/verify_native.py" "$ROOT/entry/build/default/outputs/default/entry-default-unsigned.hap"
    if [[ "${1:-run}" == run ]]; then
      one_device
      HAP="$ROOT/entry/build/default/outputs/default/entry-default-signed.hap"
      [[ -f "$HAP" ]] || { echo 'Configure local debug signing in DevEco Studio' >&2; exit 1; }
      result="$("$HDC" install -r "$HAP")"; echo "$result"
      [[ "$result" == *'install bundle successfully'* ]] || exit 1
      "$HDC" shell aa force-stop "$BUNDLE" >/dev/null 2>&1 || true
      launch
    fi
    ;;
  launch) launch;;
  *) echo 'Usage: scripts/dev.sh [build|run|launch]' >&2; exit 2;;
esac
