#!/usr/bin/env sh
# PREDICATE-SOUNDNESS FUZZER -- broad random coverage of the predicate-soundness seam: the inductive
# predicates Mem/ProdIs/Perm (the FTA's foundation) bridged to the gamma reference interpreter. Where
# predicate-diamond-fuzz cross-checks the three CHECKERS against each other, this cross-checks a kernel
# typing derivation (implementations/beta/check.beta) against an independent EXECUTABLE decision procedure (member/prod/isperm).
# For each random goal it requires implementations/beta/check.beta to ACCEPT the proof against the true goal and REJECT it
# against a perturbed goal, AND the interpreter's decision to return 1 (true) / 0 (perturbed). A
# disagreement is a checker bug or a kernel/operational soundness gap. Needs python3.
set -e
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "bootstrap paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?
. "$OMEGA_PATH_BETA/artifact_env.sh" || exit $?
cd "$OMEGA_PATH_PROOF_KERNEL"
command -v python3 >/dev/null 2>&1 || { echo "predicate-soundness fuzz: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_ALPHA}"/seed_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
ASM="${OMEGA_PATH_ALPHA_ASSEMBLER}"/$BETA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
stamp_beta_compiler "$T/bc.exe" >/dev/null
b() { "$T/bc.exe" < "$1" > "$T/x.asm" && "$ASM" < "$T/x.asm" > "$T/x.tape" && stamp_seed "$T/x.tape" "$SEED" "$2" >/dev/null 2>&1; }
b implementations/beta/check.beta "$T/check.exe"            || { echo "build implementations/beta/check.beta failed"; exit 1; }
b "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" || { echo "build interp.beta failed"; exit 1; }
python3 corpus/fuzz/predicate-soundness-fuzz.py "$T/check.exe" "$T/interp.exe" "${1:-80}"
