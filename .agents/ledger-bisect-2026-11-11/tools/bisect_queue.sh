#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")/../../.." rev-parse --show-toplevel)"
cd "$ROOT"
RUN="$ROOT/.agents/ledger-bisect-2026-11-11/tools/run_bisect.sh"
commit_cluster() {
  local id=$1
  git add .agents/ledger-bisect-2026-11-11/
  git commit -m "repo: ledger bisect part-6 cluster ${id} witness record"
  git push -u origin work/ledger-bisect-part-6
}

clusters=(
  "0:pass/providers/specialized_mixed_structural_result_operator_hosted_native"
  "1:pass/layouts/declaration_only_initializer_formation"
  "2:pass/expressions/match_operand_anonymous_operation_exit"
  "3:pass/proofs/proposition_relation_hierarchy_compile"
)

for item in "${clusters[@]}"; do
  id=${item%%:*}
  w=${item#*:}
  out="$ROOT/.agents/ledger-bisect-2026-11-11/bisect_cluster${id}.json"
  if [[ -f "$out" ]] && grep -q '"status": "bisected"' "$out" 2>/dev/null && [[ -n $(python3 -c "import json;print(json.load(open('$out')).get('breaking_commit',''))") ]]; then
    if python3 -c "import json;d=json.load(open('$out'));exit(0 if d.get('breaking_commit') and d['breaking_commit']!='51e8c45c793007f211d0d9baee7de16767172444' else 1)" 2>/dev/null; then
      echo "skip cluster $id already bisected"
      continue
    fi
  fi
  echo "=== bisect cluster $id witness $w ==="
  nice -n 19 bash "$RUN" "$w" "$out"
  python3 "$ROOT/.agents/ledger-bisect-2026-11-11/tools/write_part6.py"
  commit_cluster "$id"
done
