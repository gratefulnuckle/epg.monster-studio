#!/usr/bin/env bash
# v2 portable launcher. Data, logs, and tools live next to this repo
# (EPG_MONSTER_HOME). NSIS / Authenticode are v3.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PIDFILE="$ROOT/.studio-dev.pid"
LAUNCHABLE="$ROOT/epg-monster-studio"
ICON_PNG="$ROOT/src-tauri/icons/mascot.png"
TOOL_STATE="$ROOT/.studio-install.json"
export EPG_MONSTER_HOME="$ROOT"
cd "$ROOT"
# .cargo/config.toml pins a Windows S:\ path so GNU artifacts stay off C:.
# On Unix that string is a literal folder in the repo; override it.
case "$(uname -s)" in
  Linux|Darwin)
    if [[ -z "${CARGO_TARGET_DIR:-}" || "$CARGO_TARGET_DIR" == [A-Za-z]:* || "$CARGO_TARGET_DIR" == *\\* ]]; then
      export CARGO_TARGET_DIR="$ROOT/src-tauri/target"
    fi
    ;;
esac
PKG=""
APT_UPDATED=0
ACTION_LOG=""

if [[ -t 1 ]]; then
  C_MAG=$'\033[38;5;141m'
  C_GRN=$'\033[38;5;114m'
  C_YEL=$'\033[38;5;221m'
  C_RED=$'\033[38;5;203m'
  C_CYN=$'\033[38;5;117m'
  C_DIM=$'\033[38;5;245m'
  C_RST=$'\033[0m'
else
  C_MAG=""; C_GRN=""; C_YEL=""; C_RED=""; C_CYN=""; C_DIM=""; C_RST=""
fi

center() {
  local text="$1" width="$2"
  local len=${#text}
  if (( len >= width )); then
    printf '%s' "${text:0:width}"
    return
  fi
  local pad=$((width - len))
  local left=$((pad / 2))
  local right=$((pad - left))
  printf '%*s%s%*s' "$left" "" "$text" "$right" ""
}

UI_MODE="help"
UI_NOTE=""
UI_KEYS=()
UI_STATE=()
UI_DETAIL=()
UI_KIND=()

term_cols() {
  local w="${COLUMNS:-}"
  if [[ -z "$w" || "$w" -lt 20 ]] && [[ -t 1 ]]; then
    w="$(tput cols 2>/dev/null || true)"
  fi
  if [[ -z "$w" || "$w" -lt 60 ]]; then
    w=80
  fi
  printf '%s\n' "$w"
}

term_rows() {
  local h="${LINES:-}"
  if [[ -z "$h" || "$h" -lt 10 ]] && [[ -t 1 ]]; then
    h="$(tput lines 2>/dev/null || true)"
  fi
  if [[ -z "$h" || "$h" -lt 24 ]]; then
    h=32
  fi
  printf '%s\n' "$h"
}

QUIT_HINT="CTRL+Q to QUIT"
USE_SPLIT=0
PROMPT_TEXT=""
LOG_LINES=()
STTY_ORIG=""
if [[ "${BASH_SOURCE[0]}" == "$0" && -t 0 ]]; then
  STTY_ORIG="$(stty -g 2>/dev/null || true)"
  stty -ixon 2>/dev/null || true
fi

restore_tty() {
  printf '\033[?25h'
  if [[ -n "$STTY_ORIG" ]]; then
    stty "$STTY_ORIG" 2>/dev/null || true
  fi
}

quit_ui() {
  restore_tty
  echo
  printf '%s  quit%s\n' "$C_DIM" "$C_RST"
  exit 0
}

# Visible cell: flatten CR/LF, ellipsis if too long, pad to width. Never wraps.
clip_text() {
  local text="$1" width="$2" n
  text="${text//$'\r'/}"
  text="${text//$'\n'/ }"
  n=${#text}
  if (( width < 1 )); then
    return 0
  fi
  if (( n > width )); then
    if (( width == 1 )); then
      printf '%s' "${text:0:1}"
    else
      printf '%s…' "${text:0:$((width - 1))}"
    fi
  else
    printf '%s%*s' "$text" "$((width - n))" ""
  fi
}

step_line_text() {
  local name="$1" state="$2" detail="${3:-}" kind="$4" tag
  case "$kind" in
    ok) tag="[ ok ]" ;;
    skip) tag="[ -- ]" ;;
    warn) tag="[ !! ]" ;;
    fail) tag="[ XX ]" ;;
    *) tag="[ .. ]" ;;
  esac
  printf '  %s  %s  %s  %s' "$tag" "$(clip_text "$name" 12)" "$(clip_text "$state" 16)" "$detail"
}

# Prints: TOP LOG. TOP + 2 + LOG = HEIGHT. Log pane at least 6 rows.
pane_layout() {
  local h="$1" row_n="$2"
  local need=$((4 + 3 + 2 + row_n + 3))
  local top=$((h / 2))
  local log
  if (( need > top )); then top=$need; fi
  if (( top > h - 8 )); then top=$((h - 8)); fi
  log=$((h - top - 2))
  if (( log < 6 )); then
    log=6
    top=$((h - 8))
  fi
  if (( top < 12 && h > 18 )); then top=12; fi
  if (( top < 8 )); then top=$((h / 2)); fi
  if (( top + log + 2 != h )); then
    log=$((h - top - 2))
    if (( log < 1 )); then log=1; fi
  fi
  printf '%s %s\n' "$top" "$log"
}

seed_install_keys() {
  local os="${1:-$(uname -s)}"
  case "$os" in
    Darwin)
      printf '%s\n' "Node.js" "Rust" "Homebrew" "ffmpeg" "ffprobe" "mpv" "VLC" "npm" "data" "UI build" "cargo" "launchable"
      ;;
    *)
      printf '%s\n' "Node.js" "Rust" "cc" "WebKitGTK" "ffmpeg" "ffprobe" "mpv" "VLC" "npm" "data" "UI build" "cargo" "launchable"
      ;;
  esac
}

seed_uninstall_keys() {
  printf '%s\n' "app" "Desktop" "applications" "launchable" "Node.js" "Rust" "ffmpeg" "mpv" "VLC"
}

seed_rows_from() {
  local k
  UI_KEYS=()
  UI_STATE=()
  UI_DETAIL=()
  UI_KIND=()
  while IFS= read -r k; do
    [[ -z "$k" ]] && continue
    UI_KEYS+=("$k")
    UI_STATE+=("...")
    UI_DETAIL+=("")
    UI_KIND+=("wait")
  done
}

seed_install_rows() { seed_rows_from < <(seed_install_keys); }
seed_uninstall_rows() { seed_rows_from < <(seed_uninstall_keys); }

kind_color() {
  case "$1" in
    ok) printf '%s' "$C_GRN" ;;
    skip) printf '%s' "$C_DIM" ;;
    warn) printf '%s' "$C_YEL" ;;
    fail) printf '%s' "$C_RED" ;;
    *) printf '%s' "$C_CYN" ;;
  esac
}

ui_split_geom() {
  _H="$(term_rows)"
  _COLS="$(term_cols)"
  local row_n=0
  if [[ ${#UI_KEYS[@]} -gt 0 ]]; then row_n=${#UI_KEYS[@]}; fi
  if [[ "$USE_SPLIT" -eq 0 ]]; then
    _TOP="$_H"
    _LOG=0
    return 0
  fi
  read -r _TOP _LOG <<<"$(pane_layout "$_H" "$row_n")"
}

pane_top() {
  ui_split_geom
  printf '%s\n' "$_TOP"
}

put_at() {
  local y="$1" text="$2" color="${3:-}"
  local cols w
  cols="$(term_cols)"
  w=$((cols - 1))
  if (( w < 1 )); then w=1; fi
  text="$(clip_text "$text" "$w")"
  printf '\033[%s;1H%s%s%s' "$y" "$color" "$text" "$C_RST"
}

add_ui_log() {
  local line="$1"
  [[ -z "$line" ]] && return 0
  LOG_LINES+=("$line")
  paint_log
}

paint_log() {
  [[ "$USE_SPLIT" -eq 1 ]] || return 0
  ui_split_geom
  local start n from i idx
  start=$((_TOP + 1))
  n="$_LOG"
  if (( n < 1 )); then return 0; fi
  from=0
  if (( ${#LOG_LINES[@]} > n )); then from=$(( ${#LOG_LINES[@]} - n )); fi
  for (( i=0; i<n; i++ )); do
    idx=$((from + i))
    if (( idx < ${#LOG_LINES[@]} )); then
      put_at "$((start + i))" "${LOG_LINES[$idx]}" "$C_DIM"
    else
      put_at "$((start + i))" "" "$C_DIM"
    fi
  done
}

draw_quit_hint() {
  local cols rows msg
  ui_split_geom
  cols="$_COLS"
  msg="$QUIT_HINT"
  if [[ "$USE_SPLIT" -eq 1 ]]; then
    rows=$((_TOP - 1))
  else
    rows="$_H"
  fi
  if (( rows < 1 )); then rows=1; fi
  put_at "$rows" "$(clip_text "$(printf '%*s%s' $((cols - 1 - ${#msg})) "" "$msg")" $((cols - 1)))" "$C_DIM"
}

logo() {
  local mode="$1"
  local title="epg.monster studio"
  local sub="2026 edition  -  v2.0.2  -  $mode"
  local inner=${#sub}
  if (( ${#title} + 2 > inner )); then inner=$(( ${#title} + 2 )); fi
  inner=$((inner + 2))
  local H=$'\u2550' V=$'\u2551'
  local TL=$'\u2554' TR=$'\u2557' BL=$'\u255A' BR=$'\u255D'
  local fill="" i
  for (( i=0; i<inner; i++ )); do fill+="$H"; done
  printf '%s  %s%s%s%s\033[K\n' "$C_MAG" "$TL" "$fill" "$TR" "$C_RST"
  printf '%s  %s%s%s%s\033[K\n' "$C_MAG" "$V" "$(center "$title" "$inner")" "$V" "$C_RST"
  printf '%s  %s%s%s%s\033[K\n' "$C_MAG" "$V" "$(center "$sub" "$inner")" "$V" "$C_RST"
  printf '%s  %s%s%s%s\033[K\n' "$C_MAG" "$BL" "$fill" "$BR" "$C_RST"
}

UI_PAINTED=0
PAINT_Y=1

paint_advance() {
  local text="$1" color="${2:-}"
  local limit
  ui_split_geom
  if [[ "$USE_SPLIT" -eq 1 ]]; then
    limit=$((_TOP - 2))
    if (( PAINT_Y >= limit )); then
      return 0
    fi
  fi
  put_at "$PAINT_Y" "$text" "$color"
  PAINT_Y=$((PAINT_Y + 1))
}

write_logo() {
  local mode="$1"
  local title="epg.monster studio"
  local sub="2026 edition  -  v2.0.2  -  $mode"
  local inner=${#sub}
  if (( ${#title} + 2 > inner )); then inner=$(( ${#title} + 2 )); fi
  inner=$((inner + 2))
  local H=$'\u2550' V=$'\u2551'
  local TL=$'\u2554' TR=$'\u2557' BL=$'\u255A' BR=$'\u255D'
  local fill="" i
  for (( i=0; i<inner; i++ )); do fill+="$H"; done
  paint_advance "  ${TL}${fill}${TR}" "$C_MAG"
  paint_advance "  ${V}$(center "$title" "$inner")${V}" "$C_MAG"
  paint_advance "  ${V}$(center "$sub" "$inner")${V}" "$C_MAG"
  paint_advance "  ${BL}${fill}${BR}" "$C_MAG"
}

logo() { write_logo "$1"; }

paint() {
  printf '\033[?25l'
  if [[ "$UI_PAINTED" -eq 0 ]]; then
    printf '\033[2J'
    UI_PAINTED=1
  fi
  printf '\033[H'
  ui_split_geom
  PAINT_Y=1
  write_logo "$UI_MODE"
  paint_advance "    folder     $ROOT" "$C_DIM"
  paint_advance "    launchable $LAUNCHABLE" "$C_DIM"
  paint_advance "    data       $ROOT/data" "$C_DIM"
  paint_advance ""
  local i
  if [[ ${#UI_KEYS[@]} -gt 0 ]]; then
    for i in "${!UI_KEYS[@]}"; do
      paint_advance "$(step_line_text "${UI_KEYS[$i]}" "${UI_STATE[$i]}" "${UI_DETAIL[$i]}" "${UI_KIND[$i]}")" "$(kind_color "${UI_KIND[$i]}")"
    done
  fi
  paint_advance ""
  if [[ -n "$UI_NOTE" ]]; then
    local line color
    while IFS= read -r line; do
      color="$C_DIM"
      if [[ "$line" == *complete* ]]; then color="$C_GRN"
      elif [[ "$line" == -\>* ]]; then color="$C_CYN"
      fi
      paint_advance "  $line" "$color"
    done <<< "$UI_NOTE"
  fi
  if [[ "$USE_SPLIT" -eq 1 ]]; then
    local prompt_y quit_y div_y i rule
    prompt_y=$((_TOP - 2))
    quit_y=$((_TOP - 1))
    div_y="$_TOP"
    while (( PAINT_Y < prompt_y )); do
      put_at "$PAINT_Y" ""
      PAINT_Y=$((PAINT_Y + 1))
    done
    put_at "$prompt_y" "  $PROMPT_TEXT" "$C_YEL"
    PAINT_Y="$quit_y"
    draw_quit_hint
    rule=""
    for (( i=1; i<_COLS; i++ )); do rule+="─"; done
    put_at "$div_y" "$rule" "$C_MAG"
    paint_log
    printf '\033[%s;1H' "$prompt_y"
  else
    printf '\033[%s;1H\033[J' "$PAINT_Y"
    draw_quit_hint
  fi
}

set_row() {
  local name="$1"
  local i found=0
  if [[ ${#UI_KEYS[@]} -gt 0 ]]; then
    for i in "${!UI_KEYS[@]}"; do
      if [[ "${UI_KEYS[$i]}" == "$name" ]]; then
        UI_STATE[$i]="$2"
        UI_DETAIL[$i]="${3:-}"
        UI_KIND[$i]="$4"
        found=1
        break
      fi
    done
  fi
  if [[ "$found" -eq 0 ]]; then
    UI_KEYS+=("$name")
    UI_STATE+=("$2")
    UI_DETAIL+=("${3:-}")
    UI_KIND+=("$4")
  fi
  paint
}

COMPILE_CRATES=()
CHECK_CRATES=()
LAUNCH_TREE=()
CARGO_DONE=0

print_named_tree() {
  local title="$1"
  local keep="$2"
  shift 2
  local n=$#
  if [[ "$n" -eq 0 ]]; then
    return 0
  fi
  printf '%s            %s  (%s)%s\n' "$C_MAG" "$title" "$n" "$C_RST"
  local start=0
  if [[ "$keep" -gt 0 && "$n" -gt "$keep" ]]; then
    printf '%s            |-- ... %s earlier%s\n' "$C_DIM" "$((n - keep))" "$C_RST"
    start=$((n - keep))
  fi
  local i=0 branch item
  for item in "$@"; do
    if (( i >= start )); then
      if (( i == n - 1 )); then branch='`-- '; else branch='|-- '; fi
      printf '%s            %s%s%s\n' "$C_DIM" "$branch" "$item" "$C_RST"
    fi
    i=$((i + 1))
  done
}

print_compile_tree() {
  local keep=12
  if [[ "$CARGO_DONE" -eq 1 ]]; then keep=0; fi
  if [[ ${#COMPILE_CRATES[@]} -gt 0 ]]; then
    print_named_tree "compile" "$keep" "${COMPILE_CRATES[@]}"
  fi
  if [[ ${#CHECK_CRATES[@]} -gt 0 ]]; then
    print_named_tree "check" "$keep" "${CHECK_CRATES[@]}"
  fi
}

print_launch_tree() {
  if [[ ${#LAUNCH_TREE[@]} -gt 0 ]]; then
    print_named_tree "copy" 0 "${LAUNCH_TREE[@]}"
  fi
}

reset_ui() {
  UI_MODE="$1"
  UI_NOTE=""
  UI_KEYS=()
  UI_STATE=()
  UI_DETAIL=()
  UI_KIND=()
  COMPILE_CRATES=()
  CHECK_CRATES=()
  LAUNCH_TREE=()
  CARGO_DONE=0
  UI_PAINTED=0
}

banner() {
  reset_ui "$1"
  case "$1" in
    install) seed_install_rows ;;
    uninstall) seed_uninstall_rows ;;
  esac
  paint
}

step_line() {
  printf '%s%s%s\n' "$(kind_color "$4")" "$(step_line_text "$1" "$2" "${3:-}" "$4")" "$C_RST"
}

step() { set_row "$1" "$2" "${3:-}" "$4"; }

phase() { UI_NOTE="$1"; paint; }

usage() {
  banner "help"
  cat <<EOF
  ./studio.sh                 install + start
  ./studio.sh --install       Node, Rust, ffmpeg, mpv/VLC; Linux GTK/WebKit; build
  ./studio.sh --shortcuts     Desktop + applications menu
  ./studio.sh --uninstall     stop, remove shortcuts + launchable; optional tools
  ./studio.sh --start         build UI, run the launchable
  ./studio.sh --stop          stop
  ./studio.sh --restart       stop then start
  ./studio.sh --help

  --install uses apt-get, dnf, or pacman (sudo) on Linux, Homebrew on macOS
  --uninstall prompts for studio plus Node, Rust, ffmpeg, mpv, VLC
  ./data is never deleted
  each action writes ./install.log / ./uninstall.log / ./start.log / ...
EOF
  draw_quit_hint
}

running() {
  if [[ ! -f "$PIDFILE" ]]; then
    return 1
  fi
  local pid
  pid="$(tr -d '[:space:]' < "$PIDFILE")"
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

stop_studio() {
  local n=0
  if [[ -f "$PIDFILE" ]]; then
    local pid
    pid="$(tr -d '[:space:]' < "$PIDFILE")"
    if [[ -n "$pid" ]]; then
      kill "$pid" 2>/dev/null || true
      sleep 1
      kill -9 "$pid" 2>/dev/null || true
      if command -v pkill >/dev/null 2>&1; then
        pkill -P "$pid" 2>/dev/null || true
      fi
      n=1
    fi
    rm -f "$PIDFILE"
  fi
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      taskkill //F //IM epg-monster-studio.exe 2>/dev/null && n=1 || true
      ;;
    *)
      pkill -f "epg-monster-studio" 2>/dev/null && n=1 || true
      ;;
  esac
  if [[ "$n" -gt 0 ]]; then step "app" "stopped" "" "ok"
  else step "app" "not running" "" "skip"
  fi
}

have_cmd() { command -v "$1" >/dev/null 2>&1; }

read_prompt_char() {
  local a=""
  IFS= read -r -n 1 a || true
  if [[ "$a" == $'\x11' ]]; then
    quit_ui
  fi
  printf '%s' "$a"
}

prompt_yes() {
  local a
  PROMPT_TEXT="? $1 [Y/n] "
  paint
  printf '\033[?25h'
  a="$(read_prompt_char)"
  PROMPT_TEXT=""
  paint
  [[ -z "$a" || "$a" =~ ^[Yy] ]]
}

prompt_no() {
  local a
  PROMPT_TEXT="? $1 [y/N] "
  paint
  printf '\033[?25h'
  a="$(read_prompt_char)"
  PROMPT_TEXT=""
  paint
  [[ "$a" =~ ^[Yy] ]]
}

json_compact() {
  if [[ -f "$TOOL_STATE" ]]; then
    tr -d '\r\n' < "$TOOL_STATE" | sed 's/[[:space:]]//g'
  else
    printf '{}'
  fi
}

read_tool_how() {
  local key="$1"
  [[ -f "$TOOL_STATE" ]] || return 0
  python3 - "$TOOL_STATE" "$key" <<'PY'
import json,sys
p,k=sys.argv[1],sys.argv[2]
try:
    j=json.load(open(p,encoding="utf-8"))
except Exception:
    sys.exit(0)
tools=j.get("tools",j)
v=tools.get(k)
if isinstance(v,str):
    print(v)
elif isinstance(v,dict):
    print(v.get("how") or "")
PY
}

remember_tool() {
  local key="$1" how="$2" path="${3:-}" cmd="${4:-}"
  python3 - "$TOOL_STATE" "$key" "$how" "$path" "$cmd" <<'PY'
import json,sys,os,datetime
p,k,how,path,cmd=sys.argv[1:6]
j={"written":"","folder":os.path.dirname(os.path.abspath(p)),"tools":{}}
if os.path.isfile(p):
    try:
        raw=json.load(open(p,encoding="utf-8"))
        if isinstance(raw.get("tools"),dict):
            j=raw
        else:
            tools={}
            for kk,vv in raw.items():
                if kk in ("written","folder","tools"):
                    continue
                tools[kk]={"how":vv} if isinstance(vv,str) else dict(vv)
            j={"written":raw.get("written",""),"folder":raw.get("folder",j["folder"]),"tools":tools}
    except Exception:
        pass
rec=j.setdefault("tools",{}).setdefault(k,{})
if how: rec["how"]=how
if path: rec["path"]=path
if cmd: rec["cmd"]=cmd
j["written"]=datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
json.dump(j, open(p,"w",encoding="utf-8"), indent=2)
PY
}

forget_tool() {
  local key="$1"
  [[ -f "$TOOL_STATE" ]] || return 0
  python3 - "$TOOL_STATE" "$key" <<'PY'
import json,sys,os
p,k=sys.argv[1],sys.argv[2]
try:
    j=json.load(open(p,encoding="utf-8"))
except Exception:
    sys.exit(0)
tools=j.get("tools")
if isinstance(tools,dict):
    tools.pop(k,None)
    if not tools:
        os.remove(p)
        raise SystemExit
    j["tools"]=tools
    json.dump(j, open(p,"w",encoding="utf-8"), indent=2)
else:
    j.pop(k,None)
    rest={kk:vv for kk,vv in j.items() if kk not in ("written","folder")}
    if not rest:
        os.remove(p)
    else:
        json.dump(j, open(p,"w",encoding="utf-8"), indent=2)
PY
}

detect_pkg() {
  case "$(uname -s)" in
    Darwin)
      if have_cmd brew; then echo brew; else echo none; fi
      ;;
    Linux)
      local id="" like="" blob
      if [[ -r /etc/os-release ]]; then
        id="$(sed -n 's/^ID=//p' /etc/os-release | head -n 1 | tr -d '"')"
        like="$(sed -n 's/^ID_LIKE=//p' /etc/os-release | head -n 1 | tr -d '"')"
      fi
      blob="|$id|$like|"
      case "$blob" in
        *"|arch|"*|*"manjaro"*|*"endeavouros"*|*"artix"*) echo pacman ;;
        *"|fedora|"*|*"rhel"*|*"centos"*|*"rocky"*|*"alma"*) echo dnf ;;
        *"|debian|"*|*"ubuntu"*|*"mint"*|*"pop"*) echo apt ;;
        *)
          if have_cmd pacman; then echo pacman
          elif have_cmd apt-get; then echo apt
          elif have_cmd dnf; then echo dnf
          else echo none
          fi
          ;;
      esac
      ;;
    *) echo none ;;
  esac
}

cargo_env() {
  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
  fi
  if [[ -d "$HOME/.cargo/bin" ]]; then
    case ":$PATH:" in
      *":$HOME/.cargo/bin:"*) ;;
      *) PATH="$HOME/.cargo/bin:$PATH" ;;
    esac
  fi
}

brew_env() {
  if [[ -x /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
  elif [[ -x /usr/local/bin/brew ]]; then
    eval "$(/usr/local/bin/brew shellenv)"
  elif have_cmd brew; then
    eval "$(brew shellenv)" 2>/dev/null || true
  fi
}

ensure_brew() {
  if have_cmd brew; then
    brew_env
    return 0
  fi
  step "Homebrew" "missing" "needed for Node / Rust / ffmpeg / mpv / VLC" "fail"
  if ! prompt_yes "Install Homebrew? (the installer may ask for your password)"; then
    return 1
  fi
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  brew_env
  have_cmd brew
}

start_action_log() {
  ACTION_LOG="$ROOT/$1.log"
  USE_SPLIT=1
  PROMPT_TEXT=""
  LOG_LINES=()
  {
    echo "epg.monster studio"
    echo "action: $1"
    echo "time: $(date '+%Y-%m-%d %H:%M:%S' 2>/dev/null || date)"
    echo "folder: $ROOT"
    echo
  } > "$ACTION_LOG"
  add_ui_log "-- $1 --"
  add_ui_log "log: $ACTION_LOG"
}

log_line() {
  [[ -n "$ACTION_LOG" ]] || return 0
  printf '%s\n' "$*" >> "$ACTION_LOG"
}

log_run() {
  local title="$1" rc=0 tmp
  shift
  log_line ""
  log_line ">> $title"
  add_ui_log "$title"
  tmp="$(mktemp)"
  "$@" >"$tmp" 2>&1 || rc=$?
  if [[ -n "$ACTION_LOG" ]]; then
    cat "$tmp" >>"$ACTION_LOG" || true
  fi
  while IFS= read -r line || [[ -n "$line" ]]; do
    add_ui_log "$line"
  done < "$tmp"
  log_line "exit: $rc"
  rm -f "$tmp"
  return "$rc"
}

# log_run captures stdout/stderr, so ask for the sudo password on the TTY first.
ensure_sudo() {
  if sudo -n true 2>/dev/null; then
    return 0
  fi
  PROMPT_TEXT="sudo password for package install"
  paint
  printf '\033[?25h'
  if [[ -n "$STTY_ORIG" ]]; then
    stty "$STTY_ORIG" 2>/dev/null || true
  fi
  sudo -v
  local rc=$?
  stty -ixon 2>/dev/null || true
  PROMPT_TEXT=""
  paint
  return "$rc"
}

run_pkg_install() {
  local how="$1"
  shift
  case "$how" in
    apt)
      ensure_sudo || return 1
      if [[ "$APT_UPDATED" -eq 0 ]]; then
        log_run "apt-get update" sudo apt-get update -y
        APT_UPDATED=1
      fi
      log_run "apt-get install $*" sudo apt-get install -y "$@"
      ;;
    dnf)
      ensure_sudo || return 1
      log_run "dnf install $*" sudo dnf install -y "$@"
      ;;
    pacman)
      ensure_sudo || return 1
      log_run "pacman -S $*" sudo pacman -S --noconfirm --needed "$@"
      ;;
    brew)
      log_run "brew install $*" brew install "$@"
      ;;
    brew-cask)
      log_run "brew install --cask $*" brew install --cask "$@"
      ;;
    *)
      return 1
      ;;
  esac
}

run_pkg_remove() {
  local how="$1"
  shift
  case "$how" in
    apt|nodesource)
      ensure_sudo || return 1
      log_run "apt-get remove $*" sudo apt-get remove -y "$@"
      ;;
    dnf)
      ensure_sudo || return 1
      log_run "dnf remove $*" sudo dnf remove -y "$@"
      ;;
    pacman)
      ensure_sudo || return 1
      log_run "pacman -R $*" sudo pacman -R --noconfirm "$@"
      ;;
    brew)
      log_run "brew uninstall $*" brew uninstall "$@"
      ;;
    brew-cask)
      log_run "brew uninstall --cask $*" brew uninstall --cask "$@"
      ;;
    rustup)
      if have_cmd rustup; then
        log_run "rustup self uninstall --yes" rustup self uninstall --yes || true
        log_run "rustup self uninstall -y" rustup self uninstall -y || true
      fi
      ;;
    *)
      return 1
      ;;
  esac
}

pkg_prompt() {
  local label="$1"
  case "$PKG" in
    apt) printf 'Install %s with apt-get (sudo)?' "$label" ;;
    dnf) printf 'Install %s with dnf (sudo)?' "$label" ;;
    pacman) printf 'Install %s with pacman (sudo)?' "$label" ;;
    brew) printf 'Install %s with Homebrew?' "$label" ;;
    *) printf 'Install %s?' "$label" ;;
  esac
}

# args: label key apt_pkgs dnf_pkgs pacman_pkgs brew_pkgs [brew_cask]
offer_pkg() {
  local label="$1" key="$2"
  local apt_p="$3" dnf_p="$4" pac_p="$5" brew_p="$6" cask_p="${7:-}"
  local how pkgs q
  case "$PKG" in
    apt) how=apt; pkgs="$apt_p" ;;
    dnf) how=dnf; pkgs="$dnf_p" ;;
    pacman) how=pacman; pkgs="$pac_p" ;;
    brew)
      if [[ -n "$cask_p" ]]; then
        how=brew-cask
        pkgs="$cask_p"
      else
        how=brew
        pkgs="$brew_p"
      fi
      ;;
    *)
      echo "No package manager to install $label (need apt-get, dnf, pacman, or Homebrew)."
      return 1
      ;;
  esac
  if [[ -z "$pkgs" ]]; then
    return 1
  fi
  q="$(pkg_prompt "$label")"
  if prompt_yes "$q"; then
    # word-split package lists
    # shellcheck disable=SC2086
    if ! run_pkg_install "$how" $pkgs; then
      return 1
    fi
    remember_tool "$key" "$how"
    return 0
  fi
  return 1
}

# args: key apt_pkgs dnf_pkgs pacman_pkgs brew_pkgs [brew_cask]
uninstall_pkg() {
  local key="$1"
  local apt_p="$2" dnf_p="$3" pac_p="$4" brew_p="$5" cask_p="${6:-}"
  local how pkgs
  how="$(read_tool_how "$key" || true)"
  [[ -z "$how" ]] && how="$PKG"
  case "$how" in
    apt|nodesource) pkgs="$apt_p" ;;
    dnf) pkgs="$dnf_p" ;;
    pacman) pkgs="$pac_p" ;;
    brew) pkgs="$brew_p" ;;
    brew-cask) pkgs="${cask_p:-$brew_p}" ;;
    rustup) pkgs="" ;;
    *)
      case "$PKG" in
        apt) how=apt; pkgs="$apt_p" ;;
        dnf) how=dnf; pkgs="$dnf_p" ;;
        pacman) how=pacman; pkgs="$pac_p" ;;
        brew)
          if [[ -n "$cask_p" ]]; then how=brew-cask; pkgs="$cask_p"
          else how=brew; pkgs="$brew_p"
          fi
          ;;
        *) return 1 ;;
      esac
      ;;
  esac
  if [[ "$how" == rustup ]]; then
    run_pkg_remove rustup || true
  elif [[ -n "$pkgs" ]]; then
    # shellcheck disable=SC2086
    run_pkg_remove "$how" $pkgs || true
  else
    return 1
  fi
  forget_tool "$key"
}

have_ffmpeg() {
  local dir="$ROOT/tools/ffmpeg"
  if [[ -x "$dir/ffmpeg" && -x "$dir/ffprobe" ]]; then
    return 0
  fi
  have_cmd ffmpeg && have_cmd ffprobe
}

ffmpeg_dir() {
  local dir="$ROOT/tools/ffmpeg"
  if [[ -x "$dir/ffmpeg" && -x "$dir/ffprobe" ]]; then
    printf '%s\n' "$dir"
    return 0
  fi
  if have_cmd ffmpeg && have_cmd ffprobe; then
    dirname "$(command -v ffmpeg)"
    return 0
  fi
  return 1
}

mpv_path() {
  if have_cmd mpv; then command -v mpv; return 0; fi
  local p
  for p in /usr/bin/mpv /usr/local/bin/mpv /opt/homebrew/bin/mpv \
           "/Applications/mpv.app/Contents/MacOS/mpv"; do
    if [[ -x "$p" ]]; then printf '%s\n' "$p"; return 0; fi
  done
  return 1
}

vlc_path() {
  if have_cmd vlc; then command -v vlc; return 0; fi
  local p
  for p in /usr/bin/vlc /usr/local/bin/vlc /opt/homebrew/bin/vlc \
           "/Applications/VLC.app/Contents/MacOS/VLC"; do
    if [[ -x "$p" ]]; then printf '%s\n' "$p"; return 0; fi
  done
  return 1
}

node_major() {
  if ! have_cmd node; then
    echo 0
    return 0
  fi
  node -p "parseInt(process.versions.node,10)" 2>/dev/null || echo 0
}

install_node_unix() {
  local maj
  maj="$(node_major)"
  if [[ "$maj" -ge 22 ]]; then
    return 0
  fi
  step "Node.js" "missing" "need 22+" "fail"
  if [[ "$PKG" == apt ]]; then
    if prompt_yes "Install Node.js 22 with NodeSource (sudo apt-get)?"; then
      curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash -
      sudo apt-get install -y nodejs
      remember_tool node nodesource
      APT_UPDATED=1
    elif offer_pkg "Node.js" node "nodejs npm" "nodejs npm" "nodejs npm" "node"; then
      :
    fi
  else
    offer_pkg "Node.js" node "nodejs npm" "nodejs npm" "nodejs npm" "node" || true
  fi
}

install_rust_unix() {
  cargo_env
  if have_cmd cargo; then
    return 0
  fi
  step "Rust" "missing" "need cargo" "fail"
  if [[ "$PKG" == brew ]] && prompt_yes "Install Rust with Homebrew?"; then
    run_pkg_install brew rust || true
    remember_tool rust brew
    cargo_env
  fi
  if ! have_cmd cargo && prompt_yes "Install Rust with rustup (https://rustup.rs, no sudo)?"; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    remember_tool rust rustup
    cargo_env
  fi
  if ! have_cmd cargo; then
    offer_pkg "Rust (cargo)" rust "rustc cargo" "rust cargo" "rust" "" || true
    cargo_env
  fi
}

install_ffmpeg_unix() {
  step "ffmpeg" "missing" "need ffmpeg + ffprobe" "fail"
  if offer_pkg "ffmpeg (includes ffprobe)" ffmpeg "ffmpeg" "ffmpeg" "ffmpeg" "ffmpeg"; then
    return 0
  fi
  echo "Install ffmpeg from your package manager, then re-run --install."
  if [[ "$PKG" == dnf ]]; then
    echo "Fedora: ffmpeg is in RPM Fusion. Enable that repo, then: sudo dnf install ffmpeg"
  fi
}

linux_webkit_ok() {
  have_cmd pkg-config && { pkg-config --exists webkit2gtk-4.1 || pkg-config --exists webkit2gtk-4.0; }
}

linux_cc_ok() {
  have_cmd cc || have_cmd gcc
}

# Ubuntu 24.04 / Mint 22: libappindicator3-dev conflicts with ayatana
# (already used by the desktop). Tauri 2 builds against ayatana.
apt_tray_dev_pkg() {
  if apt-cache show libayatana-appindicator3-dev >/dev/null 2>&1; then
    printf '%s\n' libayatana-appindicator3-dev
  else
    printf '%s\n' libappindicator3-dev
  fi
}

install_build_libs_linux() {
  if [[ "$(uname -s)" != "Linux" ]]; then
    return 0
  fi
  local need_webkit=1 need_cc=1 tray
  if linux_webkit_ok; then
    need_webkit=0
    step "WebKitGTK" "ok" "" "ok"
  fi
  if linux_cc_ok; then
    need_cc=0
    step "cc" "ok" "" "ok"
  fi
  if [[ "$need_webkit" -eq 0 && "$need_cc" -eq 0 ]]; then
    return 0
  fi
  if [[ "$need_webkit" -eq 1 ]]; then
    step "WebKitGTK" "missing" "needed to compile" "fail"
  fi
  if [[ "$need_cc" -eq 1 ]]; then
    step "cc" "missing" "needed to compile" "fail"
  fi
  case "$PKG" in
    apt)
      tray="$(apt_tray_dev_pkg)"
      if prompt_yes "Install Linux build deps with apt-get (sudo)?"; then
        if ! run_pkg_install apt \
          build-essential pkg-config \
          libwebkit2gtk-4.1-dev libgtk-3-dev "$tray" \
          librsvg2-dev patchelf; then
          step "WebKitGTK" "apt failed" "see ./install.log" "fail"
        fi
      fi
      ;;
    dnf)
      if prompt_yes "Install Linux build deps with dnf (sudo)?"; then
        if ! run_pkg_install dnf gcc pkgconf-pkg-config \
          webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel \
          librsvg2-devel patchelf; then
          step "WebKitGTK" "dnf failed" "see ./install.log" "fail"
        fi
      fi
      ;;
    pacman)
      if prompt_yes "Install Linux build deps with pacman (sudo)?"; then
        if ! run_pkg_install pacman base-devel pkgconf \
          webkit2gtk-4.1 gtk3 libappindicator-gtk3 librsvg patchelf; then
          step "WebKitGTK" "pacman failed" "see ./install.log" "fail"
        fi
      fi
      ;;
    *)
      echo "See README: install libwebkit2gtk-4.1-dev libgtk-3-dev (or distro equivalent)."
      ;;
  esac
  if linux_webkit_ok; then
    step "WebKitGTK" "ok" "" "ok"
  else
    echo "Need WebKitGTK headers to compile (libwebkit2gtk-4.1-dev or distro equivalent)." >&2
    echo "See ${ACTION_LOG:-./install.log}" >&2
    exit 1
  fi
  if linux_cc_ok; then
    step "cc" "ok" "" "ok"
  else
    echo "Need a C compiler (build-essential / gcc) to compile." >&2
    exit 1
  fi
}

cargo_target_dir() {
  local td
  td="$(cargo metadata --format-version 1 --no-deps --offline --manifest-path src-tauri/Cargo.toml 2>/dev/null \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | head -n 1)"
  if [[ -n "$td" ]]; then
    printf '%s\n' "$td"
    return 0
  fi
  if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    printf '%s\n' "$CARGO_TARGET_DIR"
    return 0
  fi
  printf '%s\n' "$ROOT/src-tauri/target"
}

find_cargo_release_bin() {
  local leaf="epg-monster-studio"
  local td
  td="$(cargo_target_dir)"
  local candidates=(
    "$td/release/$leaf"
    "$ROOT/src-tauri/target/release/$leaf"
    "$ROOT/target/release/$leaf"
  )
  local p
  for p in "${candidates[@]}"; do
    if [[ -f "$p" ]]; then
      echo "$p"
      return 0
    fi
  done
  return 1
}

quiet() {
  local log="${TMPDIR:-/tmp}/epg-monster-studio-install.log"
  if ! "$@" >"$log" 2>&1; then
    UI_NOTE="command failed"
    paint
    tail -n 20 "$log" >&2 || true
    return 1
  fi
}

build_launchable() {
  set_row "UI build" "building" "npm run build" "wait"
  quiet npm run build
  set_row "UI build" "ok" "dist/" "ok"
  COMPILE_CRATES=()
  CHECK_CRATES=()
  CARGO_SEEN=""
  CARGO_DONE=0
  CARGO_TOTAL=1
  set_row "cargo" "(0/1  0%)" "starting" "wait"
  cargo_add_crate() {
    local name="$1" fresh="$2" n total pct verb
    [[ -z "$name" ]] && return 0
    if [[ "$fresh" -eq 1 ]]; then
      CHECK_CRATES+=("$name")
      verb="check"
    else
      COMPILE_CRATES+=("$name")
      verb="compile"
    fi
    case " $CARGO_SEEN " in
      *" $name "*) return 0 ;;
    esac
    CARGO_SEEN="$CARGO_SEEN $name"
    n=0
    for _ in $CARGO_SEEN; do n=$((n + 1)); done
    if [[ "$CARGO_TOTAL" -lt "$n" ]]; then CARGO_TOTAL=$n; fi
    total=$CARGO_TOTAL
    pct=0
    if [[ "$total" -gt 0 ]]; then pct=$(( 100 * n / total )); fi
    if [[ "$pct" -gt 99 && "$CARGO_DONE" -eq 0 ]]; then pct=99; fi
    set_row "cargo" "($n/$total  ${pct}%)" "$verb  $name" "wait"
  }
  local line crate fresh cargo_log cargo_rc=0
  cargo_log="$(mktemp)"
  set +e
  cargo build -p epg-monster-studio --message-format=json --release --features custom-protocol --manifest-path src-tauri/Cargo.toml >"$cargo_log" 2>&1
  cargo_rc=$?
  set -e
  log_line ""
  log_line ">> cargo build -p epg-monster-studio --release"
  if [[ -n "$ACTION_LOG" ]]; then
    cat "$cargo_log" >>"$ACTION_LOG" || true
  fi
  log_line "exit: $cargo_rc"
  while IFS= read -r line || [[ -n "$line" ]]; do
    if [[ "$line" == \{* ]]; then
      crate=""
      crate="$(printf '%s\n' "$line" | sed -n 's/.*#\([^@"]*\)@\([^"]*\)".*/\1 v\2/p' | head -n 1)"
      if [[ -z "$crate" ]]; then
        crate="$(printf '%s\n' "$line" | sed -n 's/.*"package_id":"\([^@ "]*\)@\([^"]*\)".*/\1 v\2/p' | head -n 1)"
      fi
      if [[ "$line" == *'"reason":"compiler-artifact"'* && -n "$crate" ]]; then
        fresh=0
        [[ "$line" == *'"fresh":true'* ]] && fresh=1
        cargo_add_crate "$crate" "$fresh"
      fi
    elif [[ "$line" =~ Compiling[[:space:]]+([^[:space:]]+)[[:space:]]+(v[^[:space:]]+) ]]; then
      cargo_add_crate "${BASH_REMATCH[1]} ${BASH_REMATCH[2]}" 0
    elif [[ "$line" =~ Checking[[:space:]]+([^[:space:]]+)[[:space:]]+(v[^[:space:]]+) ]]; then
      cargo_add_crate "${BASH_REMATCH[1]} ${BASH_REMATCH[2]}" 1
    fi
  done < "$cargo_log"
  rm -f "$cargo_log"
  if [[ "$cargo_rc" -ne 0 ]]; then
    set_row "cargo" "failed" "exit $cargo_rc" "fail"
    echo "cargo build failed (exit $cargo_rc). See ${ACTION_LOG:-./install.log}." >&2
    exit "$cargo_rc"
  fi
  CARGO_DONE=1
  n=$(( ${#COMPILE_CRATES[@]} + ${#CHECK_CRATES[@]} ))
  set_row "cargo" "($n/$n  100%)" "done" "ok"
  local built
  built="$(find_cargo_release_bin)" || {
    echo "cargo build finished but epg-monster-studio was not found in the target dir." >&2
    exit 1
  }
  set_row "launchable" "(1/2  50%)" "copying exe" "wait"
  cp -f "$built" "$LAUNCHABLE"
  chmod +x "$LAUNCHABLE"
  set_row "launchable" "(2/2  100%)" "ready" "ok"
}

install_studio() {
  banner "install"
  phase "toolchain"
  cargo_env
  PKG="$(detect_pkg)"
  if [[ "$(uname -s)" == Darwin ]]; then
    if ! have_cmd brew; then
      ensure_brew || true
    else
      brew_env
    fi
    PKG="$(detect_pkg)"
    if have_cmd brew; then
      step "Homebrew" "ok" "$(brew --prefix 2>/dev/null || true)" "ok"
    fi
  fi
  if [[ "$(uname -s)" == Linux && "$PKG" == none ]]; then
    echo "No apt-get, dnf, or pacman on PATH. Install Node, Rust, and ffmpeg yourself, then re-run --install." >&2
  fi

  install_node_unix
  if [[ "$(node_major)" -lt 22 ]]; then
    echo "Need Node 22+ on PATH. https://nodejs.org/" >&2
    exit 1
  fi
  step "Node.js" "ok" "$(node -v 2>/dev/null || true)" "ok"

  install_rust_unix
  if ! have_cmd cargo; then
    echo "Need Rust (cargo) on PATH. https://rustup.rs/" >&2
    exit 1
  fi
  step "Rust" "ok" "$(cargo --version 2>/dev/null || true)" "ok"

  phase "media tools"
  if ! have_ffmpeg; then
    install_ffmpeg_unix
    if ! have_ffmpeg; then
      echo "Need ffmpeg and ffprobe on PATH." >&2
      exit 1
    fi
  fi
  local ffdir
  ffdir="$(ffmpeg_dir || true)"
  step "ffmpeg" "ok" "${ffdir:+$ffdir/ffmpeg}" "ok"
  step "ffprobe" "ok" "${ffdir:+$ffdir/ffprobe}" "ok"

  local mpv vlc
  if mpv="$(mpv_path)"; then
    step "mpv" "found" "$mpv" "ok"
  else
    step "mpv" "not installed" "optional Play engine" "warn"
    if offer_pkg "mpv" mpv "mpv" "mpv" "mpv" "mpv"; then
      mpv="$(mpv_path || true)"
    fi
    if [[ -n "${mpv:-}" ]]; then step "mpv" "ok" "$mpv" "ok"
    else step "mpv" "skipped" "Play needs a path in Settings" "warn"
    fi
  fi
  if vlc="$(vlc_path)"; then
    step "VLC" "found" "$vlc" "ok"
  else
    step "VLC" "not installed" "optional Play engine" "warn"
    if offer_pkg "VLC" vlc "vlc" "vlc" "vlc" "vlc" "vlc"; then
      vlc="$(vlc_path || true)"
    fi
    if [[ -n "${vlc:-}" ]]; then step "VLC" "ok" "$vlc" "ok"
    else step "VLC" "skipped" "Play needs a path in Settings" "warn"
    fi
  fi

  install_build_libs_linux

  phase "workspace"
  if [[ ! -d node_modules ]]; then
    set_row "npm" "installing" "npm install" "wait"
    quiet npm install
    step "npm" "installed" "node_modules" "ok"
  else
    step "npm" "ok" "node_modules present" "ok"
  fi
  mkdir -p "$ROOT/data"
  step "data" "ok" "$ROOT/data" "ok"
  build_launchable
  UI_NOTE="install complete!

-> ./studio.sh --shortcuts to install desktop and menu shortcuts
->  then launch via shortcuts or use  ./studio.sh --start
log: ./install.log"
  paint
}

write_desktop_file() {
  local dest="$1"
  mkdir -p "$(dirname "$dest")"
  cat > "$dest" <<EOF
[Desktop Entry]
Type=Application
Name=epg.monster studio
Comment=epg.monster studio
Exec=env EPG_MONSTER_HOME="$ROOT" "$LAUNCHABLE"
Path=$ROOT
Icon=$ICON_PNG
Terminal=false
Categories=AudioVideo;Utility;
StartupNotify=true
EOF
  chmod +x "$dest"
  step "shortcut" "wrote" "$dest" "ok"
}

write_command_file() {
  local dest="$1"
  mkdir -p "$(dirname "$dest")"
  cat > "$dest" <<EOF
#!/bin/bash
export EPG_MONSTER_HOME="$ROOT"
cd "$ROOT"
exec "$LAUNCHABLE"
EOF
  chmod +x "$dest"
  step "shortcut" "wrote" "$dest" "ok"
}

linux_desk() {
  if have_cmd xdg-user-dir; then xdg-user-dir DESKTOP
  else printf '%s\n' "$HOME/Desktop"
  fi
}

install_shortcuts() {
  banner "shortcuts"
  if [[ ! -x "$LAUNCHABLE" ]]; then
    step "launchable" "missing" "running --install first" "warn"
    install_studio
  fi
  if [[ ! -x "$LAUNCHABLE" ]]; then
    echo "Need $LAUNCHABLE. Re-run --install." >&2
    exit 1
  fi
  case "$(uname -s)" in
    Linux)
      write_desktop_file "$HOME/.local/share/applications/epg.monster-studio.desktop"
      local desk
      desk="$(linux_desk)"
      if [[ -d "$desk" ]]; then
        write_desktop_file "$desk/epg.monster studio.desktop"
        if have_cmd gio; then
          gio set "$desk/epg.monster studio.desktop" metadata::trusted true 2>/dev/null || true
        fi
      fi
      if have_cmd update-desktop-database; then
        update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
      fi
      ;;
    Darwin)
      mkdir -p "$HOME/Applications"
      write_command_file "$HOME/Applications/epg.monster studio.command"
      if [[ -d "$HOME/Desktop" ]]; then
        write_command_file "$HOME/Desktop/epg.monster studio.command"
      fi
      ;;
    *)
      echo "On Windows use .\\studio.ps1 --shortcuts" >&2
      exit 1
      ;;
  esac
  echo
  printf '%s  shortcuts ready. data stays in %s/data%s\n' "$C_GRN" "$ROOT" "$C_RST"
  echo
}

remove_if_exists() {
  local p="$1"
  if [[ -e "$p" ]]; then
    rm -f "$p"
    step "remove" "deleted" "$p" "ok"
  else
    step "remove" "absent" "$p" "skip"
  fi
}

uninstall_studio() {
  banner "uninstall"
  PKG="$(detect_pkg)"
  cargo_env
  if [[ "$(uname -s)" == Darwin ]]; then
    brew_env
    PKG="$(detect_pkg)"
  fi
  echo
  printf '%s  What do you want to uninstall?  ./data is never deleted.%s\n' "$C_YEL" "$C_RST"
  if prompt_yes "Remove studio binary and Desktop / applications shortcuts?"; then
    phase "stop"
    stop_studio
    phase "shortcuts"
    case "$(uname -s)" in
      Linux)
        remove_if_exists "$HOME/.local/share/applications/epg.monster-studio.desktop"
        remove_if_exists "$(linux_desk)/epg.monster studio.desktop"
        ;;
      Darwin)
        remove_if_exists "$HOME/Applications/epg.monster studio.command"
        remove_if_exists "$HOME/Desktop/epg.monster studio.command"
        ;;
    esac
    phase "launchable"
    remove_if_exists "$LAUNCHABLE"
    remove_if_exists "$PIDFILE"
    step "app" "removed" "binary + shortcuts" "ok"
  else
    step "app" "kept" "" "skip"
  fi

  if have_cmd node && prompt_no "Uninstall Node.js too?"; then
    uninstall_pkg node "nodejs npm" "nodejs npm" "nodejs npm" "node" || true
    step "Node.js" "removed" "" "ok"
  else
    step "Node.js" "kept" "" "skip"
  fi

  cargo_env
  if { have_cmd cargo || have_cmd rustup; } && prompt_no "Uninstall Rust (cargo / rustup) too?"; then
    uninstall_pkg rust "rustc cargo" "rust cargo" "rust" "rust" || true
    step "Rust" "removed" "" "ok"
  else
    step "Rust" "kept" "" "skip"
  fi

  if have_ffmpeg && prompt_no "Uninstall ffmpeg/ffprobe too?"; then
    uninstall_pkg ffmpeg "ffmpeg" "ffmpeg" "ffmpeg" "ffmpeg" || true
    step "ffmpeg" "removed" "" "ok"
  else
    step "ffmpeg" "kept" "" "skip"
  fi

  if mpv_path >/dev/null && prompt_no "Uninstall mpv too?"; then
    uninstall_pkg mpv "mpv" "mpv" "mpv" "mpv" || true
    step "mpv" "removed" "" "ok"
  else
    step "mpv" "kept" "" "skip"
  fi

  if vlc_path >/dev/null && prompt_no "Uninstall VLC too?"; then
    uninstall_pkg vlc "vlc" "vlc" "vlc" "vlc" "vlc" || true
    step "VLC" "removed" "" "ok"
  else
    step "VLC" "kept" "" "skip"
  fi

  UI_NOTE="uninstall complete

./data was not deleted.
log: ./uninstall.log"
  paint
}

start_studio() {
  if running; then
    step "app" "already running" "pid $(tr -d '[:space:]' < "$PIDFILE")" "ok"
    return 0
  fi
  export EPG_MONSTER_HOME="$ROOT"
  if [[ -x "$LAUNCHABLE" ]]; then
    phase "build UI (dist/)"
    npm run build
    phase "start"
    "$LAUNCHABLE" &
    echo $! > "$PIDFILE"
    step "app" "started" "pid $(tr -d '[:space:]' < "$PIDFILE")" "ok"
    return 0
  fi
  step "launchable" "missing" "cargo run (or --install for a release binary)" "warn"
  phase "build UI (dist/)"
  npm run build
  cargo run --features custom-protocol --manifest-path src-tauri/Cargo.toml &
  echo $! > "$PIDFILE"
  step "app" "started" "pid $(tr -d '[:space:]' < "$PIDFILE") cargo run" "ok"
}

# Sourced by scripts/test-studio-ui.sh — do not install or touch the TTY.
if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
  return 0
fi

want_install=0
want_shortcuts=0
want_uninstall=0
want_start=0
want_stop=0
want_help=0
unknown=()
if [[ $# -eq 0 ]]; then
  want_install=1
  want_start=1
else
  for a in "$@"; do
    case "$a" in
      --install | install ) want_install=1 ;;
      --shortcuts | shortcuts ) want_shortcuts=1 ;;
      --uninstall | uninstall ) want_uninstall=1 ;;
      --start | start ) want_start=1 ;;
      --stop | stop ) want_stop=1 ;;
      --restart | restart ) want_stop=1; want_start=1 ;;
      --help | -h | help ) want_help=1 ;;
      * ) unknown+=("$a") ;;
    esac
  done
fi
if [[ ${#unknown[@]} -gt 0 ]]; then
  echo "unknown: ${unknown[*]}" >&2
  usage >&2
  exit 1
fi
if [[ "$want_help" -eq 1 ]]; then
  usage
  exit 0
fi
trap 'restore_tty' EXIT
if [[ "$want_stop" -eq 1 ]]; then start_action_log stop; stop_studio; fi
if [[ "$want_uninstall" -eq 1 ]]; then start_action_log uninstall; uninstall_studio; fi
if [[ "$want_install" -eq 1 ]]; then start_action_log install; install_studio; fi
if [[ "$want_shortcuts" -eq 1 ]]; then start_action_log shortcuts; install_shortcuts; fi
if [[ "$want_start" -eq 1 ]]; then start_action_log start; start_studio; fi
