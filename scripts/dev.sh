#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB="$ROOT/upstream/photocraft/apps/photocraft-web"
SOURCE="$ROOT/upstream/photocraft"
PORT=8765
BUNDLE=io.github.storytold.photocraft.pad
HDC="${PHOTOCRAFT_HDC:-/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc}"
HVIGOR="${PHOTOCRAFT_HVIGOR:-/Applications/DevEco-Studio.app/Contents/tools/hvigor/bin/hvigorw}"
TRUNK="${PHOTOCRAFT_TRUNK:-$(command -v trunk || true)}"
TRUNK_ARGS=(serve --skip-version-check --watch "$SOURCE/crates" --watch "$WEB" --watch "$SOURCE/Cargo.toml" --watch "$SOURCE/Cargo.lock" --poll --poll-interval 1000ms)
export DEVECO_SDK_HOME="${DEVECO_SDK_HOME:-/Applications/DevEco-Studio.app/Contents/sdk}"
export JAVA_HOME="${JAVA_HOME:-/Applications/DevEco-Studio.app/Contents/jbr/Contents/Home}"

serve() {
  if [[ -z "$TRUNK" || ! -x "$TRUNK" ]]; then
    echo 'Trunk is missing. Install trunk 0.21.14 or set PHOTOCRAFT_TRUNK.' >&2
    exit 1
  fi
  cd "$WEB"
  exec env -u NO_COLOR "$TRUNK" "${TRUNK_ARGS[@]}"
}

launch() {
  if [[ "$("$HDC" fport ls)" != *"tcp:$PORT tcp:$PORT"* ]]; then
    "$HDC" rport "tcp:$PORT" "tcp:$PORT"
  fi
  "$HDC" shell aa force-stop "$BUNDLE" >/dev/null 2>&1 || true
  "$HDC" shell aa start -a EntryAbility -b "$BUNDLE" --pb photocraft.dev true
}

case "${1:-run}" in
  serve)
    serve
    ;;
  launch)
    launch
    ;;
  run)
    if [[ -z "$TRUNK" || ! -x "$TRUNK" ]]; then
      echo 'Trunk is missing. Install trunk 0.21.14 or set PHOTOCRAFT_TRUNK.' >&2
      exit 1
    fi
    cd "$ROOT"
    "$HVIGOR" assembleHap --mode module -p product=default -p buildMode=debug --no-daemon
    HAP="$ROOT/entry/build/default/outputs/default/entry-default-signed.hap"
    if [[ ! -f "$HAP" ]]; then
      echo 'Signed HAP is unavailable. Configure DevEco Studio debug signing first.' >&2
      exit 1
    fi
    INSTALL_RESULT="$("$HDC" install -r "$HAP")"
    echo "$INSTALL_RESULT"
    if [[ "$INSTALL_RESULT" != *'install bundle successfully'* ]]; then
      echo 'HAP installation failed. If the simulator has a differently signed copy, remove it after backing up app data.' >&2
      exit 1
    fi
    cd "$WEB"
    env -u NO_COLOR "$TRUNK" "${TRUNK_ARGS[@]}" &
    SERVER_PID=$!
    trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT INT TERM
    for ((i=0; i<600; i++)); do
      if curl --silent --fail "http://127.0.0.1:$PORT/index.html" >/dev/null; then
        break
      fi
      if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        wait "$SERVER_PID"
        exit 1
      fi
      sleep 1
    done
    curl --silent --fail "http://127.0.0.1:$PORT/index.html" >/dev/null
    launch
    echo 'PhotoCraft is running from source. Keep this terminal open for automatic rebuilds.'
    wait "$SERVER_PID"
    ;;
  *)
    echo 'Usage: scripts/dev.sh [run|serve|launch]' >&2
    exit 2
    ;;
esac
