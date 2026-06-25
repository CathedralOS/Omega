#!/usr/bin/env sh
# ELABORATOR regression — the untrusted proof elaborator (elab.py) compiles named-binder
# proof sources (proofs/*.elab) to raw certificates, which the TRUSTED check.beta must
# accept. This keeps the productivity tool honest: a bug in the elaborator that emitted a
# malformed or wrong-indexed certificate would make check.beta reject. The elaborator is
# NOT in the trust path (it only produces certificates the minimal checker re-validates),
# exactly as beta-lang-rs was throwaway scaffolding for bc.
cd "$(dirname "$0")"
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
../beta-lang-rs/build/bc.exe < check.beta > "$T/p.asm" || { echo "bc(check.beta) failed"; exit 1; }
"$ASM" < "$T/p.asm" > "$T/p.tape" || { echo "asm failed"; exit 1; }
stamp_seed "$T/p.tape" "$SEED" "$T/check.exe" >/dev/null 2>&1
PASS=0; FAIL=0
for f in proofs/*.elab; do
  out=$(python3 elab.py --check "$T/check.exe" < "$f" 2>&1)
  if [ "$out" = accept ]; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); echo "  FAIL $f : $out"; fi
done
echo "elaborator regression (named-binder sources -> elaborate -> check.beta accepts): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
