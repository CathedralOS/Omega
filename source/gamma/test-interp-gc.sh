#!/usr/bin/env sh
# Focused Gamma non-moving-GC teeth. These deliberately cross the 40 MiB
# dynamic heap boundary and stay separate from the fast semantic suite.
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
"$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED" < "$T/interp.asm" > "$T/interp.tape"
stamp_seed "$T/interp.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$T/interp.exe" >/dev/null 2>&1

HELPERS='(def ntht (t k h) (if (eq h 0) t (match t ((Node l r) (if (lt k h) (ntht l k (/ h 2)) (ntht r (- k h) (/ h 2)))) (z 0))))
(def nth (xs k) (match xs ((Chunks n t) (ntht t k 262144))))
(def sett (t k v h) (if (eq h 0) v (match t ((Node l r) (if (lt k h) (Node (sett l k v (/ h 2)) r) (Node l (sett r (- k h) v (/ h 2))))) (z (if (lt k h) (Node (sett 0 k v (/ h 2)) 0) (Node 0 (sett 0 (- k h) v (/ h 2))))))))
(def setl (xs k v) (match xs ((Chunks n t) (Chunks n (sett t k v 262144)))))
(def churn (n xs) (if (eq n 0) xs (churn (- n 1) (setl xs 0 (% n 256)))))'

run42() {
  LABEL=$1
  PROGRAM=$2
  START=$(date +%s)
  set +e
  printf '%s\n%s' "$HELPERS" "$PROGRAM" | "$T/interp.exe" > "$T/$LABEL.stdout"
  STATUS=$?
  set -e
  ELAPSED=$(($(date +%s) - START))
  [ "$STATUS" -eq 42 ] && [ "$(cat "$T/$LABEL.stdout")" = 42 ] || {
    echo "gamma interp GC: $LABEL observed status $STATUS output '$(cat "$T/$LABEL.stdout")'" >&2
    exit 1
  }
  echo "gamma interp GC: $LABEL passed after reclaiming persistent versions (${ELAPSED}s)"
}

# Each iteration allocates nineteen compact Node paths plus one Chunks wrapper:
# 132,000 * 320 = 42,240,000 dynamic bytes, unconditionally beyond the complete
# 40 MiB heap. Only the current version remains live. `old` must survive as both
# an environment root and a child of the already-
# evaluated first Pair argument while evaluation of the second argument triggers
# collection. `fresh` shares the old persistent tree but updates a different lane.
run42 alias-and-temporary-root \
  '(def verify (p) (match p ((Pair held fresh) (match held ((Hold old marker) (+ marker (+ (nth old 1) (nth fresh 0)))))))) (let old (setl (Chunks 524288 0) 1 7) (verify (Pair (Hold old 34) (churn 132000 old))))'

echo "gamma interp GC: reclaimable Chunks, live alias, and Beta temporary roots passed"
