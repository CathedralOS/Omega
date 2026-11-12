#!/bin/bash
# Newest first-parent ancestor of PIN where fixed-ledger witness test is 0.
set -uo pipefail
PIN="${1:?pin}"
SCRIPT="${2:?witness script}"
ROOT="$(git -C "$(dirname "$0")/../../.." rev-parse --show-toplevel)"
cd "$ROOT"
chmod -R u+w "$ROOT/target" 2>/dev/null || true
find "$ROOT/target" -type f ! -writable -exec chmod u+w {} + 2>/dev/null || true

nth_fp() {
  local n=$1
  git rev-list --first-parent "$PIN" | sed -n "$((n + 1))p"
}

is_ledger_only_commit() {
  git rev-parse -q --verify HEAD^1 >/dev/null 2>&1 || return 1
  mapfile -t files < <(git diff --name-only HEAD^1 HEAD)
  ((${#files[@]} == 0)) && return 1
  for f in "${files[@]}"; do
    if [[ "$f" != tests/omega/corpus_outcomes.txt ]] && [[ ! "$f" =~ \.md$ ]]; then
      return 1
    fi
  done
  return 0
}

test_at() {
  local n=$1
  local cur
  cur=$(nth_fp "$n")
  [[ -n "$cur" ]] || return 125
  git checkout -q "$cur"
  if is_ledger_only_commit; then
    return 125
  fi
  chmod -R u+w "$ROOT/target" 2>/dev/null || true
  set +e
  "$SCRIPT" >/dev/null 2>&1
  local rc=$?
  echo "find_good n=$n $(git rev-parse --short HEAD) rc=$rc" >&2
  return "$rc"
}

test_at 0
pin_rc=$?
[[ "$pin_rc" -eq 1 ]] || { echo "PIN is not bad (rc=$pin_rc) under fixed ledger context" >&2; exit 1; }

hint=$(git log --first-parent -1 --format=%H -S "${BISECT_WITNESS}" -- tests/omega/corpus_outcomes.txt 2>/dev/null || true)
max_n=$(git rev-list --first-parent --count "$PIN")
max_n=$((max_n - 1))
hi=$max_n
if [[ -n "$hint" ]]; then
  hi=$(git rev-list --first-parent --count "${hint}..${PIN}" 2>/dev/null || echo "$max_n")
  if [[ "$hi" -gt "$max_n" ]]; then
    hi=$max_n
  fi
  # confirm hint side is good
  hint_n=$hi
  test_at "$hint_n"
  hint_rc=$?
  if [[ "$hint_rc" -ne 0 ]]; then
    hi=$max_n
  fi
fi

lo=0
while ((lo + 1 < hi)); do
  mid=$(( (lo + hi) / 2 ))
  test_at "$mid"
  rc=$?
  if [[ "$rc" -eq 0 ]]; then
    hi=$mid
  elif [[ "$rc" -eq 1 ]]; then
    lo=$mid
  else
    lo=$((mid + 1))
  fi
done

for ((try = lo; try <= hi; try++)); do
  test_at "$try"
  rc=$?
  if [[ "$rc" -eq 0 ]]; then
    nth_fp "$try"
    exit 0
  fi
done
exit 1
