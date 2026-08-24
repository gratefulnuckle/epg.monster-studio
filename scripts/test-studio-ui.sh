#!/usr/bin/env bash
# Layout contract for studio.sh two-pane UI. Each check names a break:
# wrap, drifting divider, growing step table, header scrolled off.
set -euo pipefail
export LC_ALL=C.UTF-8
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=../studio.sh
source "$ROOT/studio.sh"

fail=0
assert_eq() {
  local got="$1" want="$2" msg="$3"
  if [[ "$got" != "$want" ]]; then
    printf 'FAIL %s\n  got:  %q\n  want: %q\n' "$msg" "$got" "$want" >&2
    fail=1
  fi
}

# Break: clip_text longer than the cell wraps and shoves the header off-screen.
got="$(clip_text "abcdefghijklmnopqrstuvwxyz" 10)"
assert_eq "${#got}" "10" "clip_text length equals width"
case "$got" in
  *abcdefghijklmnopqrstuvwxyz*) 
    printf 'FAIL clip_text still contains the unclipped tail\n' >&2
    fail=1
    ;;
esac
assert_eq "$(clip_text $'ab\ncd' 8)" "$(printf '%-8s' 'ab cd')" "clip_text flattens newlines then pads"

# Break: step columns drift because name/state widths are not fixed, or detail wraps.
COLUMNS=80
line="$(step_line_text "Node.js" "ok" "/very/long/path/that/would/wrap/the/terminal/if-not-clipped" "ok")"
assert_eq "${line//$'\n'/}" "$line" "step_line_text has no newline"
# tag + 2 spaces + name12 + 2 + state16 + 2 + detail…  (no trailing newline in value)
# "  [ ok ]  " is 10 chars; name 12; "  "; state 16
assert_eq "${line:2:6}" "[ ok ]" "step tag field"
assert_eq "${line:10:12}" "Node.js     " "step name is a 12-char column"
assert_eq "${line:24:16}" "ok              " "step state is a 16-char column"
clipped="$(clip_text "$line" 80)"
assert_eq "${#clipped}" "80" "clipped step line fills exactly 80 columns"

# Break: pane_top grows with 2 lines per row and the log pane collapses / header scrolls.
read -r top log <<<"$(pane_layout 32 13)"
assert_eq "$((top + log + 2))" "32" "pane_layout top + divider gap + log = height"
if (( log < 6 )); then
  printf 'FAIL log pane is %s, want >= 6 on a 32-line terminal\n' "$log" >&2
  fail=1
fi
if (( top < 12 )); then
  printf 'FAIL top pane is %s, want >= 12 on a 32-line terminal\n' "$top" >&2
  fail=1
fi
# 13 step rows must not force top past height-8 (24 on 32-line).
if (( top > 24 )); then
  printf 'FAIL top pane is %s, want <= height-8 so the log still fits\n' "$top" >&2
  fail=1
fi

# Break: install grid grows as tools are found, so the divider jumps.
mapfile -t linux_keys < <(seed_install_keys Linux)
assert_eq "${#linux_keys[@]}" "13" "Linux install grid is 13 fixed rows"
assert_eq "${linux_keys[0]}" "Node.js" "first Linux install row"
assert_eq "${linux_keys[3]}" "WebKitGTK" "Linux has WebKitGTK, not Homebrew"
assert_eq "${linux_keys[-1]}" "launchable" "last Linux install row"
mapfile -t mac_keys < <(seed_install_keys Darwin)
assert_eq "${#mac_keys[@]}" "12" "macOS install grid is 12 fixed rows"
assert_eq "${mac_keys[2]}" "Homebrew" "macOS has Homebrew, not WebKitGTK"

if [[ "$fail" -ne 0 ]]; then
  exit 1
fi
printf 'ok studio-ui layout\n'
