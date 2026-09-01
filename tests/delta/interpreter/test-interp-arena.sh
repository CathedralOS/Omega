#!/usr/bin/env sh
# Focused Delta interpreter fixed-arena capacity tooth. Kept separate from the
# fast semantic suite because deliberately retaining more than the complete
# 16 MiB arena takes seconds.
set -eu

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"

T=$(mktemp -d)
trap 'rm -rf -- "$T"' EXIT
stamp_gamma_compiler "$T/gc.exe" >/dev/null
"$T/gc.exe" < "$OMEGA_REPO_ROOT/tests/delta/interpreter/interp.gamma" > "$T/delta_interpreter_bytecode.tape"
stamp_seed "$T/delta_interpreter_bytecode.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$T/interp.exe" >/dev/null 2>&1

# Eight compact Cons allocations per tail step retain 17,920,000 bytes. The
# exact checked boundary must remain status 254 with no partial output.
PROGRAM='(def fill (n xs) (if (eq n 0) 0 (fill (- n 1) (Cons 0 (Cons 0 (Cons 0 (Cons 0 (Cons 0 (Cons 0 (Cons 0 (Cons 0 xs))))))))))) (fill 140000 Nil)'
START=$(date +%s)
set +e
printf '%s' "$PROGRAM" | "$T/interp.exe" > "$T/stdout"
STATUS=$?
set -e
ELAPSED=$(($(date +%s) - START))

[ "$STATUS" -eq 254 ] || {
  echo "delta interp arena: status $STATUS, expected 254" >&2
  exit 1
}
[ ! -s "$T/stdout" ] || {
  echo "delta interp arena: exhaustion published partial output" >&2
  exit 1
}
echo "delta interp arena: checked exhaustion returned 254 without output (${ELAPSED}s)"
