#!/bin/bash
# Exponential first-parent walk from PIN; first exit-0 is good for bisect. Never-good past fixture-add.
set -euo pipefail
WITNESS="${1:?witness}"
MAIN_REPO="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
ROOT="${BISECT_REPO_ROOT:-$MAIN_REPO}"
PREPARE="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/tools/prepare_bisect_expect.py"
SCRIPT="$MAIN_REPO/.agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
PIN="$(git -C "$ROOT" rev-parse HEAD)"
cd "$ROOT"
START="$PIN"
DEPTH="${BISECT_GOOD_DEPTH:-1200}"

export_file="$(
  BISECT_PIN_LEDGER="${BISECT_PIN_LEDGER:-/tmp/bisect_pin_corpus_outcomes.txt}" \
    python3 "$PREPARE" "$WITNESS"
)"
export BISECT_EXPECT_FILE="$export_file"
export BISECT_WITNESS="$WITNESS"
export BISECT_REPO_ROOT="$ROOT"

mapfile -t COMMITS < <(git rev-list --first-parent -n "$DEPTH" "$PIN")
n=${#COMMITS[@]}

fixture_add_sha() {
  local rel="tests/omega/${WITNESS}.omg"
  local sha=""
  if git cat-file -e "${rel}^{commit}" 2>/dev/null; then
    sha="$(git log -1 --diff-filter=A --format=%H --first-parent -- "$rel" 2>/dev/null || true)"
  fi
  if [[ -z "$sha" ]]; then
    sha="$(git log -1 --diff-filter=A --format=%H --first-parent -S"${WITNESS##*/}" -- tests/omega samples 2>/dev/null || true)"
  fi
  echo "$sha"
}

test_commit() {
  local commit="$1"
  git checkout -f "$commit" >/dev/null 2>&1 || return 125
  local rc=0
  nice -n 19 "$SCRIPT" >/dev/null 2>&1 || rc=$?
  return "$rc"
}

FIX_ADD="$(fixture_add_sha)"
FIDX=-1
if [[ -n "$FIX_ADD" ]]; then
  for i in "${!COMMITS[@]}"; do
    if [[ "${COMMITS[$i]}" == "$FIX_ADD" ]]; then
      FIDX=$i
      break
    fi
  done
fi

offset=1
while ((offset < n)); do
  rc=0
  test_commit "${COMMITS[$offset]}" || rc=$?
  if [[ "$rc" -eq 0 ]]; then
    git checkout -f "$START" >/dev/null 2>&1
    echo "${COMMITS[$offset]}"
    exit 0
  fi
  if [[ "$FIDX" -ge 0 && "$offset" -ge "$FIDX" ]]; then
    git checkout -f "$START" >/dev/null 2>&1
    echo "NEVER_GOOD:${FIX_ADD}" >&2
    exit 2
  fi
  offset=$((offset * 2))
done

if [[ "$FIDX" -ge 0 ]]; then
  git checkout -f "$START" >/dev/null 2>&1
  echo "NEVER_GOOD:${FIX_ADD}" >&2
  exit 2
fi

git checkout -f "$START" >/dev/null 2>&1
exit 1
