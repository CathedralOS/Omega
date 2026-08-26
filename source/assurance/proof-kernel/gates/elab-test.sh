#!/usr/bin/env sh
# ELABORATOR regression — the untrusted proof elaborator (tools/elab.py) compiles named-binder
# proof sources (corpus/proofs/*.elab) to raw certificates, which the TRUSTED implementations/beta/check.beta must
# accept. This keeps the productivity tool honest: a bug in the elaborator that emitted a
# malformed or wrong-indexed certificate would make implementations/beta/check.beta reject. The elaborator is
# NOT in the trust path (it only produces certificates the minimal checker re-validates),
# exactly as other untrusted certificate producers remain outside the trust path.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
. "$OMEGA_PATH_BETA/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
"$T/bc.exe" < implementations/beta/check.beta > "$T/p.asm" || { echo "bc(implementations/beta/check.beta) failed"; exit 1; }
"$ASM" < "$T/p.asm" > "$T/p.tape" || { echo "asm failed"; exit 1; }
stamp_seed "$T/p.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1
PASS=0; FAIL=0
for f in corpus/proofs/*.elab; do
  out=$(python3 tools/elab.py --check "$T/check.exe" < "$f" 2>&1)
  if [ "$out" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $f : $out"; fi
done
echo "elaborator regression (named-binder sources -> elaborate -> implementations/beta/check.beta accepts): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
