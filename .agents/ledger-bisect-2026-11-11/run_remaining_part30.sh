#!/bin/bash
set -euo pipefail
export PATH="/home/ubuntu/.local/share/mbx/bin:$PATH"
REPO=/agent/repos/omega
PIN=51e8c45c793007f211d0d9baee7de16767172444
cd "$REPO"
git checkout -qf work/ledger-bisect-part-30
chmod +x .agents/ledger-bisect-2026-11-11/*.sh

run_one() {
  local tag="$1" witness="$2" anchor="${3:-$PIN}"
  if ! grep -q "| $tag | bisected |" .agents/ledger-bisect-2026-11-11/part-30.md 2>/dev/null; then
    if grep "${witness}" .agents/ledger-bisect-2026-11-11/part-30.md | grep -q bisected; then
      echo "skip $tag already bisected"
      return 0
    fi
  fi
  if grep "| \`pass" .agents/ledger-bisect-2026-11-11/part-30.md | grep "$witness" | grep -q bisected; then
    echo "skip $tag"
    return 0
  fi
  bash .agents/ledger-bisect-2026-11-11/complete_cluster.sh "$tag" "$witness" "$anchor" "$PIN"
  git add .agents/ledger-bisect-2026-11-11/part-30.md .agents/ledger-bisect-2026-11-11/part-30.tsv
  git diff --cached --quiet || git commit -m "repo: ledger bisect part 30 cluster ${tag}"
  git pull --rebase origin work/ledger-bisect-part-30
  git push origin work/ledger-bisect-part-30
}

# recoverable
if ! grep recoverable_replacement .agents/ledger-bisect-2026-11-11/part-30.md | grep -q 'bisected\|never-good'; then
  run_one recoverable pass/facts/recoverable_replacement_window_through_const_name_exit 8e7394363d14aca7dae993adeb86fc348c0d2c7e
fi

# traits
if ! grep runtime_qualified_owned .agents/ledger-bisect-2026-11-11/part-30.md | grep -q bisected; then
  if bash .agents/ledger-bisect-2026-11-11/probe_traits_part17.sh "$PIN"; then
    git add .agents/ledger-bisect-2026-11-11/part-30.md .agents/ledger-bisect-2026-11-11/part-30.tsv
    git diff --cached --quiet || git commit -m "repo: ledger bisect part 30 cluster traits"
    git pull --rebase origin work/ledger-bisect-part-30
    git push origin work/ledger-bisect-part-30
  else
    run_one traits pass/traits/runtime_qualified_owned_requirement_calls_exit "$PIN"
  fi
fi

# gui
if ! grep gui_semantic .agents/ledger-bisect-2026-11-11/part-30.md | grep -q 'bisected\|never-good'; then
  run_one gui pass/effects/gui_semantic_services "$PIN"
fi
echo ALL_DONE
