#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")/../../.." rev-parse --show-toplevel)"
cd "$ROOT"
RUN="$ROOT/.agents/ledger-bisect-2026-11-11/tools/run_bisect.sh"
WRITE="$ROOT/.agents/ledger-bisect-2026-11-11/tools/write_part6.py"

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
  echo "=== cluster $id witness $w ==="
  nice -n 19 bash "$RUN" "$w" "$out"
  python3 "$WRITE"
  git add .agents/ledger-bisect-2026-11-11/part-6.md .agents/ledger-bisect-2026-11-11/part-6.tsv ".agents/ledger-bisect-2026-11-11/bisect_cluster${id}.json"
  git commit -m "repo: ledger bisect part-6 cluster ${id} witness result"
  git push origin work/ledger-bisect-part-6
done
