#!/bin/bash
set -euo pipefail
REV="${1:?}"
ROOT="${OMEGA_REPO:-/agent/repos/omega}"
mapfile -t files < <(git -C "$ROOT" diff --name-only "${REV}^" "$REV" 2>/dev/null || true)
if ((${#files[@]} == 0)); then
  exit 1
fi
for f in "${files[@]}"; do
  case "$f" in
    tests/omega/corpus_outcomes.txt) ;;
    *.md) ;;
    *) exit 1 ;;
  esac
done
exit 0
