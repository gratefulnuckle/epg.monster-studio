#!/usr/bin/env bash
# Map an OpenSpec change folder to GitHub issues.
# Usage: ./scripts/openspec-gh.sh <change-folder> [--dry-run] [--no-close]
set -euo pipefail
CHANGE=""
DRY_RUN=0
CLOSE_DONE=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --no-close) CLOSE_DONE=0; shift ;;
    --change|-Change) CHANGE="${2:-}"; shift 2 ;;
    -*) echo "unknown flag: $1" >&2; exit 1 ;;
    *) CHANGE="$1"; shift ;;
  esac
done
if [[ -z "$CHANGE" ]]; then
  echo "usage: ./scripts/openspec-gh.sh <openspec/changes folder> [--dry-run] [--no-close]" >&2
  exit 1
fi
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIR="$ROOT/openspec/changes/$CHANGE"
if [[ ! -d "$DIR" ]]; then
  echo "missing change folder: $DIR" >&2
  exit 1
fi

ensure_label() {
  local name="$1" color="$2" desc="$3"
  if gh label list --json name --jq '.[].name' | grep -Fxq "$name"; then
    return 0
  fi
  gh label create "$name" --color "$color" --description "$desc" >/dev/null 2>&1 || true
}
ensure_label openspec 0E8A16 "OpenSpec change"
ensure_label v2 1D76DB "v2 tester scope"
ensure_label v3 5319E7 "parked for v3"
ensure_label install-scripts FBCA04 "studio.ps1 / studio.sh"
ensure_label tracking C5DEF5 "process / board, not product UI"

gh_create() {
  local title="$1" body="$2"
  shift 2
  local args=(issue create --title "$title" --body "$body")
  local lab
  for lab in "$@"; do
    args+=(--label "$lab")
  done
  gh "${args[@]}"
}

MAP="$DIR/github.md"
EPIC=""
declare -A TASK_MAP=()
if [[ -f "$MAP" ]]; then
  while IFS= read -r line || [[ -n "$line" ]]; do
    if [[ "$line" =~ ^epic:[[:space:]]*([0-9]+) ]]; then
      EPIC="${BASH_REMATCH[1]}"
      continue
    fi
    if [[ "$line" =~ ^\|[[:space:]]*task[[:space:]]*\| ]]; then continue; fi
    if [[ "$line" =~ ^\|[[:space:]]*-+[[:space:]]*\| ]]; then continue; fi
    if [[ "$line" =~ ^\|[[:space:]]*(.+)[[:space:]]*\|[[:space:]]*([0-9]+)[[:space:]]*\| ]]; then
      task="${BASH_REMATCH[1]}"
      task="${task%"${task##*[![:space:]]}"}"
      task="${task#"${task%%[![:space:]]*}"}"
      TASK_MAP["$task"]="${BASH_REMATCH[2]}"
    fi
  done < "$MAP"
fi

TASK_LABELS=(openspec v2)
EPIC_LABELS=(openspec v2 tracking)
if [[ "$CHANGE" == *install* ]]; then
  TASK_LABELS+=(install-scripts)
  EPIC_LABELS+=(install-scripts)
fi
if [[ "$CHANGE" == *v3* || "$CHANGE" == *ghoul* ]]; then
  TASK_LABELS=(openspec v3)
  EPIC_LABELS=(openspec v3 tracking)
fi

if [[ -z "$EPIC" ]]; then
  BODY="OpenSpec change: \`openspec/changes/${CHANGE}/\`"
  if [[ -f "$DIR/proposal.md" ]]; then
    BODY="$BODY

$(head -n 80 "$DIR/proposal.md")"
  fi
  BODY="$BODY

Do not put access keys or provider stream URLs in comments."
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "would create epic: [openspec] $CHANGE"
    EPIC="0"
  else
    URL="$(gh_create "[openspec] $CHANGE" "$BODY" "${EPIC_LABELS[@]}")"
    EPIC="${URL##*/}"
    echo "epic #$EPIC $URL"
  fi
fi

close_mapped() {
  local num="$1"
  [[ -z "$num" || "$num" == "0" ]] && return 0
  if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "would close #$num"
    return 0
  fi
  local state
  state="$(gh issue view "$num" --json state --jq .state 2>/dev/null || true)"
  if [[ "$state" != "OPEN" ]]; then
    return 0
  fi
  gh issue close "$num" --comment "Checkbox marked done in openspec/changes/${CHANGE}/tasks.md." >/dev/null
  echo "closed #$num"
}

TASKS_FILE="$DIR/tasks.md"
ORDERED=()
DONE_FLAGS=()
if [[ -f "$TASKS_FILE" ]]; then
  while IFS= read -r line || [[ -n "$line" ]]; do
    case "$line" in
      "- [ ] "*)
        ORDERED+=("${line#- [ ] }")
        DONE_FLAGS+=(0)
        ;;
      "- [x] "*|"- [X] "*)
        t="${line#- [x] }"
        t="${t#- [X] }"
        ORDERED+=("$t")
        DONE_FLAGS+=(1)
        ;;
    esac
  done < "$TASKS_FILE"
fi

i=0
for t in "${ORDERED[@]+"${ORDERED[@]}"}"; do
  done_flag="${DONE_FLAGS[$i]}"
  i=$((i + 1))
  if [[ -z "${TASK_MAP[$t]+x}" ]]; then
    if [[ "$done_flag" -eq 1 ]]; then
      continue
    fi
    TASK_BODY="Parent: #$EPIC
Change: \`openspec/changes/${CHANGE}/\`
Task: $t"
    if [[ "$DRY_RUN" -eq 1 ]]; then
      echo "would create task: $t"
      TASK_MAP["$t"]="0"
    else
      url="$(gh_create "[$CHANGE] $t" "$TASK_BODY" "${TASK_LABELS[@]}")"
      num="${url##*/}"
      TASK_MAP["$t"]="$num"
      echo "task #$num $t"
    fi
  fi
  if [[ "$CLOSE_DONE" -eq 1 && "$done_flag" -eq 1 ]]; then
    close_mapped "${TASK_MAP[$t]}"
  fi
done

TMP="$(mktemp)"
{
  echo "# GitHub mapping"
  echo
  echo "epic: $EPIC"
  echo
  echo "| task | issue |"
  echo "|------|-------|"
  declare -A WRITTEN=()
  for t in "${ORDERED[@]+"${ORDERED[@]}"}"; do
    if [[ -n "${TASK_MAP[$t]+x}" ]]; then
      echo "| $t | ${TASK_MAP[$t]} |"
      WRITTEN["$t"]=1
    fi
  done
  for t in "${!TASK_MAP[@]}"; do
    if [[ -z "${WRITTEN[$t]+x}" ]]; then
      echo "| $t | ${TASK_MAP[$t]} |"
    fi
  done
} > "$TMP"
mv "$TMP" "$MAP"
echo "wrote $MAP"
