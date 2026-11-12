#!/bin/bash
# Newest first-parent ancestor of PIN where fixed-ledger witness test is 0.
set -uo pipefail
PIN="${1:?pin}"
SCRIPT="${2:?witness script}"
ROOT="${BISECT_REPO_ROOT:-$(git rev-parse --show-toplevel)}"
export BISECT_REPO_ROOT="$ROOT"
cd "$ROOT"

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
  git checkout -qf "$cur"
  if is_ledger_only_commit; then
    return 125
  fi
  if [[ -d "$ROOT/target" ]]; then
    find "$ROOT/target" \( -type f -o -type d \) ! -perm -u+w -exec chmod u+w {} + 2>/dev/null || true
    chmod -R u+w "$ROOT/target" 2>/dev/null || true
  fi
  set +e
  "$SCRIPT" >/dev/null 2>&1
  local rc=$?
  echo "find_good n=$n $(git rev-parse --short HEAD) rc=$rc" >&2
  return "$rc"
}

test_at 0
pin_rc=$?
[[ "$pin_rc" -eq 1 ]] || { echo "PIN is not bad (rc=$pin_rc) under fixed ledger context" >&2; exit 1; }

max_n=$(git rev-list --first-parent --count "$PIN")
max_n=$((max_n - 1))

last_bad=0
for ((n = 1; n <= max_n; n++)); do
  test_at "$n"
  rc=$?
  if [[ "$rc" -eq 0 ]]; then
    nth_fp "$n"
    exit 0
  fi
  if [[ "$rc" -eq 1 ]]; then
    last_bad=$n
  fi
done

echo "no good commit found (last_bad n=$last_bad)" >&2
exit 1
