#!/bin/bash
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$ROOT"
RUNNER=$(ls -t "$ROOT"/target/debug/build/compiler/*/out/corpus_runner-* | head -1)
export OMEGA_CORPUS_ROOT="$ROOT" OMEGA_LIBRARY_ROOT="$ROOT/source/library"
export OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_SECS="${OMEGA_CORPUS_FIXTURE_SECS:-3600}"
OUT="$ROOT/wiki/drafts/audits/corpus/host_settled.txt"
mkdir -p "$(dirname "$OUT")"
HOST=(
  fail/build/installed_arrival_without_context
  fail/build/installed_arrival_wrong_controller
  pass/calls/runtime_composed_index_case_pool_transition_exit
  pass/calls/runtime_indexed_case_pool_transition_exit
  pass/filesystem/create_dir_all_symlink_kind_exit
  pass/filesystem/cross_linux_value_syscalls
  pass/filesystem/dir_walk_wrappers_exit
  pass/filesystem/runtime_portable_unknown_directory_kinds_exit
)
if [[ "${1:-}" == "--retry-timeouts" ]]; then
  HOST=(
    fail/build/installed_arrival_without_context
    fail/build/installed_arrival_wrong_controller
    pass/calls/runtime_composed_index_case_pool_transition_exit
    pass/calls/runtime_indexed_case_pool_transition_exit
    pass/filesystem/runtime_portable_unknown_directory_kinds_exit
  )
  : > "$OUT.retry"
  OUT="$OUT.retry"
fi
for fix in "${HOST[@]}"; do
  echo "host-settle $fix (${OMEGA_CORPUS_FIXTURE_SECS}s cap)"
  export OMEGA_CORPUS_FIXTURE_FILTER="$fix" OMEGA_CORPUS_RECORDS=/tmp/host_one.txt
  nice -n 19 "$RUNNER" < /dev/null || true
  cat /tmp/host_one.txt >> "$OUT"
done
