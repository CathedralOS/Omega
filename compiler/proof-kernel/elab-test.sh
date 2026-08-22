#!/usr/bin/env sh
# ELABORATOR regression — the untrusted proof elaborator (elab.py) compiles named-binder
# proof sources (proofs/*.elab) to raw certificates, which the TRUSTED check.beta must
# accept. This keeps the productivity tool honest: a bug in the elaborator that emitted a
# malformed or wrong-indexed certificate would make check.beta reject. The elaborator is
# NOT in the trust path (it only produces certificates the minimal checker re-validates),
# exactly as beta-lang-rs was throwaway scaffolding for bc.
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
cd "$OMEGA_GATE_DIR"
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_BETA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA_LANGUAGE}"/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
"${OMEGA_PATH_BETA_RUST}"/build/bc.exe < check.beta > "$T/p.asm" || { echo "bc(check.beta) failed"; exit 1; }
"$ASM" < "$T/p.asm" > "$T/p.tape" || { echo "asm failed"; exit 1; }
stamp_seed "$T/p.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1
PASS=0; FAIL=0
for f in proofs/*.elab; do
  out=$(python3 elab.py --check "$T/check.exe" < "$f" 2>&1)
  if [ "$out" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $f : $out"; fi
done
echo "elaborator regression (named-binder sources -> elaborate -> check.beta accepts): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
