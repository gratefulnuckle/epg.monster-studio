#!/usr/bin/env bash
# Build the GStreamer sidecar and stage it to {app}/tools/ghoul/.
set -euo pipefail
REPO="$(cd "$(dirname "$0")/.." && pwd)"
APP_ROOT="${1:-$REPO}"
MANIFEST="$REPO/src-tauri/crates/ghoul-gst/Cargo.toml"
DEST="$APP_ROOT/tools/ghoul"
mkdir -p "$DEST"
cargo build --release --manifest-path "$MANIFEST"
BIN="ghoul-gst"
if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* || "$(uname -s)" == CYGWIN* ]]; then
  BIN="ghoul-gst.exe"
fi
SRC="$REPO/src-tauri/target/release/$BIN"
if [[ ! -f "$SRC" && -n "${CARGO_TARGET_DIR:-}" ]]; then
  SRC="$CARGO_TARGET_DIR/release/$BIN"
fi
cp "$SRC" "$DEST/$BIN"
echo "staged $DEST/$BIN"
