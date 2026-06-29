#!/usr/bin/env sh
# CHECKER-DIAMOND FUZZER — broad random coverage of the two trust-anchor checkers agreeing. checker-
# diamond.sh cross-checks check.beta (Beta) and checker.gamma (Gamma, on the reference interpreter) at
# ~83 curated certificates; this generates hundreds of random closed Peano/List equality propositions
# (with refl proofs) and requires BOTH checkers to ACCEPT the true ones and REJECT the perturbed ones.
# A disagreement is a bug (or a backdoor) in one of the two independent checkers -- the most important
# place the lattice's "trust by checking" thesis can fail. Deterministic (fixed seed). Needs python3.
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "checker-diamond fuzz: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta "$T/check.exe"            || { echo "build check.beta failed"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
python3 checker-diamond-fuzz.py "$T/check.exe" "$T/interp.exe" ../gamma/checker.gamma "${1:-80}"
