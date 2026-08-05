#!/usr/bin/env sh
# PREDICATE-SOUNDNESS FUZZER -- broad random coverage of the predicate-soundness seam: the inductive
# predicates Mem/ProdIs/Perm (the FTA's foundation) bridged to the gamma reference interpreter. Where
# predicate-diamond-fuzz cross-checks the three CHECKERS against each other, this cross-checks a kernel
# typing derivation (check.beta) against an independent EXECUTABLE decision procedure (member/prod/isperm).
# For each random goal it requires check.beta to ACCEPT the proof against the true goal and REJECT it
# against a perturbed goal, AND the interpreter's decision to return 1 (true) / 0 (perturbed). A
# disagreement is a checker bug or a kernel/operational soundness gap. Needs python3.
set -e
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "predicate-soundness fuzz: skipped (python3 absent)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null ) || { echo "bc build failed"; exit 1; }
b() { ../beta-lang-rs/build/bc.exe < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b check.beta "$T/check.exe"            || { echo "build check.beta failed"; exit 1; }
b ../gamma/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
python3 predicate-soundness-fuzz.py "$T/check.exe" "$T/interp.exe" "${1:-80}"
