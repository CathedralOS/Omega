#!/usr/bin/env sh
# SEAM FUZZER — broad random coverage of the gamma/delta soundness seam (semantics-diamond.sh is the
# hand-picked version). The checker's DEFINITIONAL equality (eq.beta: normalize both sides) and the
# reference interpreter's OPERATIONAL evaluation (interp.beta, running gamma's own plus/mult) must
# agree on whether two closed Peano terms are equal -- two independent routes to "e1 = e2". This runs
# that comparison over MANY randomly generated +/* expressions instead of a curated handful: for each
# it checks both `E = value(E)` (must agree "equal") and `E = value(E)+1` (must agree "differ"). A
# single disagreement would be a soundness break at the seam (not a proof of the open theorem, but the
# broadest bounded evidence for it). Deterministic (fixed seed). Needs python3; skips cleanly without.
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "seam fuzz: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b eq.beta "$T/eq.exe"                  || { echo "build eq.beta failed"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
python3 seam-fuzz.py "$T/eq.exe" "$T/interp.exe" "${1:-120}"
