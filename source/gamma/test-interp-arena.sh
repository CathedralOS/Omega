#!/usr/bin/env sh
# Focused Gamma interpreter retained-live capacity tooth. Kept separate from
# the fast semantic and reclaimable-GC suites because deliberately retaining
# more than the complete 40 MiB heap takes seconds.
set -eu

OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$OMEGA_PATH_PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh"
. "$OMEGA_PATH_BETA_COMPILER/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
"$T/bc.exe" < "$OMEGA_PATH_GAMMA/interp.beta" > "$T/interp.asm"
"$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED" < "$T/interp.asm" > "$T/gamma_interpreter_bytecode.tape"
stamp_seed "$T/gamma_interpreter_bytecode.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$T/interp.exe" >/dev/null 2>&1

# Eight compact Cons allocations per tail step retain 49,664,000 bytes. No
# collector may reclaim this reachable list; the exact checked boundary must
# therefore remain status 254 with no partial output.
PROGRAM='(def fill (n xs) (if (eq n 0) 0 (fill (- n 1) (Cons 0 (Cons 0 (Cons 0 (Cons 0 (Cons 0 (Cons 0 (Cons 0 (Cons 0 xs))))))))))) (fill 388000 Nil)'
START=$(date +%s)
set +e
printf '%s' "$PROGRAM" | "$T/interp.exe" > "$T/stdout"
STATUS=$?
set -e
ELAPSED=$(($(date +%s) - START))

[ "$STATUS" -eq 254 ] || {
  echo "gamma interp arena: status $STATUS, expected 254" >&2
  exit 1
}
[ ! -s "$T/stdout" ] || {
  echo "gamma interp arena: exhaustion published partial output" >&2
  exit 1
}
echo "gamma interp arena: checked exhaustion returned 254 without output (${ELAPSED}s)"
