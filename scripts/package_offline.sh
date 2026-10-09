#!/usr/bin/env bash
# Native HAP is always offline; there is no web resource packaging step.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
case "${1:-}" in
  '') exec "$ROOT/scripts/dev.sh" build;;
  --install) exec "$ROOT/scripts/dev.sh" run;;
  *) echo 'Usage: scripts/package_offline.sh [--install]' >&2; exit 2;;
esac
