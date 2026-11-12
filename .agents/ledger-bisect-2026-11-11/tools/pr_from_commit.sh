#!/bin/bash
# Print PR number from first-parent merge subject, if any.
set -euo pipefail
sha="${1:?}"
subj="$(git log -1 --format=%s "$sha")"
if [[ "$subj" =~ \(#([0-9]+)\) ]]; then
  echo "${BASH_REMATCH[1]}"
fi
