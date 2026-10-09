#!/usr/bin/env bash
# Build the current PhotoCraft web sources and install them as the offline HAP assets.
#
# Usage: scripts/package_offline.sh [--skip-build] [--assets-only] [--install]
#
# Default: trunk release build, replace rawfile, update Index.ets,
# verify, then assemble a signed debug HAP. --assets-only stops before the HAP.
# --install also installs that HAP on the hdc target (disconnect the simulator first).
# Resolve this file before nounset: zsh rejects BASH_SOURCE, and some terminals run the file with zsh.
if [[ -n "${ZSH_VERSION:-}" ]]; then
  SCRIPT_PATH="${(%):-%x}"
else
  SCRIPT_PATH="${BASH_SOURCE[0]:-$0}"
fi
set -euo pipefail

ROOT="$(cd "$(dirname "$SCRIPT_PATH")/.." && pwd)"
if [[ -f "$ROOT/scripts/dev.local.env" ]]; then
  # Local tool paths are machine specific and must not be committed.
  # shellcheck disable=SC1091
  source "$ROOT/scripts/dev.local.env"
fi

WEB="$ROOT/upstream/photocraft/apps/photocraft-web"
DIST="$ROOT/upstream/photocraft/dist/web"
RAWFILE="$ROOT/entry/src/main/resources/rawfile"
TRUNK="${PHOTOCRAFT_TRUNK:-$(command -v trunk || true)}"
HVIGOR="${PHOTOCRAFT_HVIGOR:-/Applications/DevEco-Studio.app/Contents/tools/hvigor/bin/hvigorw}"
HDC="${PHOTOCRAFT_HDC:-/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/toolchains/hdc}"
export DEVECO_SDK_HOME="${DEVECO_SDK_HOME:-/Applications/DevEco-Studio.app/Contents/sdk}"
export JAVA_HOME="${JAVA_HOME:-/Applications/DevEco-Studio.app/Contents/jbr/Contents/Home}"

SKIP_BUILD=0
ASSETS_ONLY=0
INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --skip-build) SKIP_BUILD=1 ;;
    --assets-only) ASSETS_ONLY=1 ;;
    --install) INSTALL=1 ;;
    -h|--help)
      sed -n '2,8p' "$SCRIPT_PATH"
      exit 0
      ;;
    *)
      echo "Unknown option: $arg" >&2
      echo "Usage: scripts/package_offline.sh [--skip-build] [--assets-only] [--install]" >&2
      exit 2
      ;;
  esac
done

if [[ "$SKIP_BUILD" -eq 0 ]]; then
  if [[ -z "$TRUNK" || ! -x "$TRUNK" ]]; then
    echo "Trunk is missing. Install trunk 0.21.14 or set PHOTOCRAFT_TRUNK." >&2
    exit 1
  fi
  if ! rustc --print sysroot >/dev/null 2>&1; then
    echo "rustc is missing. Trunk must use a Rust that has the wasm32-unknown-unknown target." >&2
    exit 1
  fi
  if [[ ! -d "$(rustc --print sysroot)/lib/rustlib/wasm32-unknown-unknown" ]]; then
    echo "wasm32-unknown-unknown is not installed for $(rustc --version)." >&2
    echo "Run: rustup target add wasm32-unknown-unknown" >&2
    exit 1
  fi
  echo "Building PhotoCraft web release. The first build can take several minutes."
  (
    cd "$WEB"
    env -u NO_COLOR "$TRUNK" build --release --skip-version-check
  )
fi

python3 - "$ROOT" "$DIST" "$RAWFILE" <<'PY'
import re
import shutil
import sys
from pathlib import Path

root = Path(sys.argv[1])
dist = Path(sys.argv[2])
rawfile = Path(sys.argv[3])
max_wasm = 24 * 1024 * 1024

html = dist / "index.html"
if not html.is_file():
    raise SystemExit(f"missing {html}; run without --skip-build")
text = html.read_text(encoding="utf-8")
if re.search(r'(?:src|href)="/[^/]', text):
    raise SystemExit(f"{html} has root-absolute URLs; ArkWeb serves the package from a private origin")

scripts = sorted(dist.glob("photocraft-web-*.js"))
wasms = sorted(dist.glob("photocraft-web-*_bg.wasm"))
scripts = [p for p in scripts if not p.name.endswith("_bg.js")]
if len(scripts) != 1 or len(wasms) != 1:
    raise SystemExit(f"expected one JS and one WASM in {dist}, found { [p.name for p in scripts + wasms] }")
script, wasm = scripts[0], wasms[0]
if script.name not in text or wasm.name not in text:
    raise SystemExit("index.html does not reference the built JS/WASM names")
size = wasm.stat().st_size
print(f"{wasm.name}: {size} bytes ({size / 1048576:.1f} MiB; limit {max_wasm})")
if size > max_wasm:
    raise SystemExit(f"{wasm.name} is over the 24 MiB offline package limit")

rawfile.mkdir(parents=True, exist_ok=True)
for old in rawfile.iterdir():
    # arkdata/utd/utd.json5 registers open-with file types and is not a web asset.
    if old.name == "arkdata" and old.is_dir():
        continue
    if old.is_file():
        old.unlink()
    elif old.name != ".DS_Store":
        raise SystemExit(f"unexpected directory in rawfile: {old}")
shutil.copy2(html, rawfile / "index.html")
shutil.copy2(script, rawfile / script.name)
shutil.copy2(wasm, rawfile / wasm.name)

page = root / "entry/src/main/ets/pages/Index.ets"
page_text = page.read_text(encoding="utf-8")
updated = re.sub(
    r"const SCRIPT_NAME: string = 'photocraft-web-[a-f0-9]+\.js';",
    f"const SCRIPT_NAME: string = '{script.name}';",
    page_text,
    count=1,
)
updated = re.sub(
    r"const WASM_NAME: string = 'photocraft-web-[a-f0-9]+_bg\.wasm';",
    f"const WASM_NAME: string = '{wasm.name}';",
    updated,
    count=1,
)
if updated == page_text or script.name not in updated or wasm.name not in updated:
    raise SystemExit("failed to update SCRIPT_NAME/WASM_NAME in Index.ets")
page.write_text(updated, encoding="utf-8")

cargo = (root / "upstream/photocraft/Cargo.toml").read_text(encoding="utf-8")
version = re.search(r'(?m)^version = "([^"]+)"', cargo)
photocraft_version = version.group(1) if version else "unknown"
print(f"Installed offline assets for PhotoCraft {photocraft_version}: {script.name}, {wasm.name}")
PY

python3 "$ROOT/scripts/verify_assets.py"

if [[ "$ASSETS_ONLY" -eq 1 ]]; then
  echo "Offline assets updated. Rebuild the HAP in DevEco, or rerun without --assets-only."
  exit 0
fi

echo "Assembling signed debug HAP."
(
  cd "$ROOT"
  "$HVIGOR" assembleHap --mode module -p product=default -p buildMode=debug --no-daemon
)
HAP="$ROOT/entry/build/default/outputs/default/entry-default-signed.hap"
if [[ ! -f "$HAP" ]]; then
  echo "Assets are updated, but the signed HAP is missing. Configure DevEco debug signing and build again." >&2
  exit 1
fi
echo "Signed HAP: $HAP"
if [[ "$INSTALL" -eq 1 ]]; then
  INSTALL_RESULT="$("$HDC" install -r "$HAP")"
  echo "$INSTALL_RESULT"
  if [[ "$INSTALL_RESULT" != *'install bundle successfully'* ]]; then
    echo "HAP installation failed. If another signature is installed, uninstall that copy first." >&2
    exit 1
  fi
  echo "Installed. Launch from the tablet home screen. Do not pass photocraft.dev; that uses Trunk instead of this package."
fi
