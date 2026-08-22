#!/usr/bin/env sh
# D0 STORAGE MEANING — run the fixed-backing allocator canary through the
# lower-rung, Rust-free Delta/Omega -> Gamma elaborator and Gamma interpreter.
#
# The Rust on-ramp remains available as a native regression producer, but it is
# not used to define this slice's result. The separate bc cold-start task still
# governs how the Beta executables used here acquire lower-rooted authority.
set -e
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

( cd "${OMEGA_PATH_BETA_RUST}" && sh build.sh "${OMEGA_PATH_BETA_LANGUAGE}"/bc.beta >/dev/null ) \
  || { echo "delta storage meaning FAIL — bc build"; exit 1; }

build_beta() {
  "${OMEGA_PATH_BETA_RUST}"/build/bc.exe < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}

build_beta ${OMEGA_PATH_OMEGA0}/omega2gamma.beta "$T/elaborate.exe" \
  || { echo "delta storage meaning FAIL — omega2gamma build"; exit 1; }
build_beta "${OMEGA_PATH_GAMMA}"/interp.beta "$T/interp.exe" \
  || { echo "delta storage meaning FAIL — Gamma interpreter build"; exit 1; }

PASS=0
FAIL=0
run_meaning() {
  label=$1
  source=$2
  expected=$3
  if ! "$T/elaborate.exe" < "$source" > "$T/program.gamma" 2>"$T/elaborate.err"; then
    FAIL=$((FAIL+1)); echo "  FAIL $label : elaboration failed"; return
  fi
  if [ ! -s "$T/program.gamma" ] || grep -q 'E2G-UNSUPPORTED' "$T/program.gamma"; then
    FAIL=$((FAIL+1)); echo "  FAIL $label : unsupported or empty elaboration"; return
  fi
  set +e
  "$T/interp.exe" < "$T/program.gamma" > "$T/result" 2>/dev/null
  got=$?
  set -e
  if [ "$got" = "$expected" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1)); echo "  FAIL $label : meaning exit $got, expected $expected"
  fi
}

run_meaning_input() {
  label=$1
  source=$2
  input=$3
  expected=$4
  if ! "$T/elaborate.exe" < "$source" > "$T/program.gamma" 2>"$T/elaborate.err"; then
    FAIL=$((FAIL+1)); echo "  FAIL $label : elaboration failed"; return
  fi
  if [ ! -s "$T/program.gamma" ] || grep -q 'E2G-UNSUPPORTED' "$T/program.gamma"; then
    FAIL=$((FAIL+1)); echo "  FAIL $label : unsupported or empty elaboration"; return
  fi
  bytes=$(od -An -tu1 < "$input" | tr ' ' '\n' | grep -vE '^$' | tr '\n' ' ')
  reverse=""; for byte in $bytes; do reverse="$byte $reverse"; done
  list=Nil; for byte in $reverse; do list="(Cons $byte $list)"; done
  sed "s/STDIN/$list/" "$T/program.gamma" > "$T/program-input.gamma"
  set +e
  "$T/interp.exe" < "$T/program-input.gamma" > "$T/result" 2>/dev/null
  got=$?
  set -e
  if [ "$got" = "$expected" ]; then
    PASS=$((PASS+1))
  else
    FAIL=$((FAIL+1)); echo "  FAIL $label : meaning exit $got, expected $expected"
  fi
}

run_meaning "allocator contract" samples/bootstrap-storage.alp 42

# Teeth: perturb the durable-cell check. The elaborated meaning must follow the
# changed source and take the canary's distinguished failure state.
sed 's/== 67/== 68/' samples/bootstrap-storage.alp > "$T/perturbed.alp"
run_meaning "perturbed allocator observation" "$T/perturbed.alp" 14

printf 'OMG0BNDL\001\000\000\000\001\000\000\000\010\000\000\000\003\000\000\000main.omgabc' > "$T/bundle-ok"
cp "$T/bundle-ok" "$T/bundle-trailing"; printf x >> "$T/bundle-trailing"
printf 'OMG0BNDL\001\000\000\000\001\000\000\000\101\000\000\000\000\000\000\000' > "$T/bundle-exhausted"
printf 'OMG0BNDL\001\000\000\000\002\000\000\000\005\000\000\000\000\000\000\000z.omg\005\000\000\000\000\000\000\000a.omg' > "$T/bundle-order"
run_meaning_input "canonical Omega0 bundle" samples/omega0-bundle-decode.alp "$T/bundle-ok" 80
run_meaning_input "Omega0 bundle exact EOF" samples/omega0-bundle-decode.alp "$T/bundle-trailing" 251
run_meaning_input "Omega0 bundle checked exhaustion" samples/omega0-bundle-decode.alp "$T/bundle-exhausted" 252
run_meaning_input "Omega0 bundle canonical order" samples/omega0-bundle-decode.alp "$T/bundle-order" 251

echo "delta D0 storage meaning (omega2gamma.beta -> interp.beta): $PASS passed, $FAIL failed"
[ "$FAIL" = 0 ]
