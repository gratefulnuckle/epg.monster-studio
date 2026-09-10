#!/usr/bin/env bash
# Regression tests for studio.sh cargo target-dir resolution.
# Windows .cargo/config.toml uses S:\... which cargo treats as a relative path
# on Linux/macOS; --shortcuts then cannot find the release binary.
set -euo pipefail
REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"
# shellcheck source=../studio.sh
source "$REPO/studio.sh"
if [[ "$ROOT" != "$REPO" ]]; then
  echo "FAIL sourced studio.sh ROOT is $ROOT, expected $REPO"
  exit 1
fi

fail=0
check() {
  local name="$1" got="$2" want="$3"
  if [[ "$got" != "$want" ]]; then
    echo "FAIL $name"
    echo "  got:  $got"
    echo "  want: $want"
    fail=$((fail + 1))
  else
    echo "ok $name"
  fi
}

assert_no_windows_path() {
  local name="$1" got="$2"
  if [[ "$got" == *\\* ]] || [[ "$got" =~ (^|/)[A-Za-z]: ]]; then
    echo "FAIL $name returned a Windows cargo target path:"
    echo "  $got"
    fail=$((fail + 1))
  else
    echo "ok $name ($got)"
  fi
}

unset CARGO_TARGET_DIR || true

# Live metadata from the committed .cargo/config.toml must not leak S:\...
if command -v cargo >/dev/null 2>&1; then
  unset CARGO_METADATA_JSON || true
  got="$(cargo_target_dir)"
  assert_no_windows_path "live cargo_target_dir" "$got"
  if [[ "$got" != /* ]]; then
    echo "FAIL live cargo_target_dir is not absolute: $got"
    fail=$((fail + 1))
  fi
  check "live cargo_target_dir is src-tauri/target" "$got" "$ROOT/src-tauri/target"
fi

# Injected Windows target_directory (what cargo metadata emits from config.toml)
export CARGO_METADATA_JSON='{"target_directory":"S:\\toolchains\\cargo-target\\epg-monster-studio"}'
got="$(cargo_target_dir)"
check "windows drive metadata falls back" "$got" "$ROOT/src-tauri/target"

# Glued path cargo actually reports on this Linux checkout
export CARGO_METADATA_JSON="$(printf '%s' "{\"target_directory\":\"$ROOT/S:\\\\toolchains\\\\cargo-target\\\\epg-monster-studio\"}")"
got="$(cargo_target_dir)"
check "glued windows metadata falls back" "$got" "$ROOT/src-tauri/target"

export CARGO_METADATA_JSON='{"target_directory":"/tmp/epg-monster-studio-cargo-target"}'
got="$(cargo_target_dir)"
check "unix metadata is used" "$got" "/tmp/epg-monster-studio-cargo-target"

unset CARGO_METADATA_JSON || true
apply_cargo_target_dir
check "apply_cargo_target_dir exports fallback" "${CARGO_TARGET_DIR:-}" "$ROOT/src-tauri/target"
if command -v cargo >/dev/null 2>&1; then
  meta_td="$(python3 - "$ROOT" <<'PY'
import json, subprocess, sys
root = sys.argv[1]
out = subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline",
     "--manifest-path", root + "/src-tauri/Cargo.toml"],
    text=True,
)
print(json.loads(out).get("target_directory") or "")
PY
)"
  assert_no_windows_path "cargo metadata after apply" "$meta_td"
fi
unset CARGO_TARGET_DIR || true

need="$(msrv)"
check "msrv from Cargo.toml" "$need" "1.77"

tmp="$(mktemp -d)"
mkdir -p "$tmp/release"
touch "$tmp/release/epg-monster-studio"
export CARGO_TARGET_DIR="$tmp"
got="$(find_cargo_release_bin)"
check "find_cargo_release_bin uses CARGO_TARGET_DIR" "$got" "$tmp/release/epg-monster-studio"
rm -rf "$tmp"
unset CARGO_TARGET_DIR || true

if [[ "$fail" -ne 0 ]]; then
  echo
  echo "$fail check(s) failed"
  exit 1
fi
echo
echo "all cargo-target checks passed"
