#!/bin/bash
set -euo pipefail
ROOT="${LEDGER_BISECT_ROOT:-}"
if [[ -z "$ROOT" ]]; then
  ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null || true)"
fi
[[ -n "$ROOT" ]] || { echo "LEDGER_BISECT_ROOT unset" >&2; exit 1; }
TD="${CARGO_TARGET_DIR:-$ROOT/target-pin}"
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TD" mbx build -p compiler < /dev/null >/dev/null 2>&1 || true
CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR="$TD" \
  mbx test -p compiler --test corpus_runner --no-run --message-format=json < /dev/null 2>/dev/null \
  | python3 -c "import json,sys
last=''
for line in sys.stdin:
    line=line.strip()
    if not line:
        continue
    try:
        msg=json.loads(line)
    except json.JSONDecodeError:
        continue
    ex=msg.get('executable')
    if ex:
        last=ex
if last:
    print(last)
" > /tmp/corpus_runner_path.txt || true
if [[ -s /tmp/corpus_runner_path.txt ]]; then
  cat /tmp/corpus_runner_path.txt
  exit 0
fi
find "$TD/debug/build/compiler" -name 'corpus_runner-*' -type f -executable ! -name '*.d' -printf '%T@ %p\n' 2>/dev/null \
  | sort -n | tail -1 | cut -d' ' -f2-
