#!/bin/bash
# Best-effort PR number from merge commit on first-parent main containing $1
set -euo pipefail
C="${1:?}"
# Squash merges on main: subject line often has (#NNN)
SUBJ=$(git show -s --format='%s' "$C")
if [[ "$SUBJ" =~ \(#([0-9]+)\) ]]; then
  echo "${BASH_REMATCH[1]}"
  exit 0
fi
# Walk first-parent until an merge with PR in subject
git rev-list --first-parent "$C"..51e8c45c793007f211d0d9baee7de16767172444 2>/dev/null | while read -r m; do
  s=$(git show -s --format='%s' "$m" 2>/dev/null || true)
  if [[ "$s" =~ \(#([0-9]+)\) ]]; then
    echo "${BASH_REMATCH[1]}"
    exit 0
  fi
done
echo "none"
