#!/usr/bin/env sh
# EXHAUSTIVE I/O CORRECTNESS — for input-reading programs, verify bc compiles correctly over the WHOLE
# bounded input domain, not just sampled inputs. For each random single-byte-reading program (io-fuzz-gen.py),
# compile it once with bc and run it against the reference interpreter for EVERY input byte 0..255
# (io-verify.py), requiring the same exit code and stdout on all 256. This is a complete correctness check
# for single-byte-input programs — the fuzz's "for all inputs" analogue — and it exercises the read_byte /
# write_byte / input-dependent-branch codegen that the fixed-input correctness fuzz does not.
# Deterministic (fixed base seed). Needs python3 plus the persisted lattice bc
# artifact and assembler; skips cleanly when the host cannot run them.
OMEGA_GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$OMEGA_GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" ]; do
    OMEGA_PATH_PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    if [ "$OMEGA_PATH_PARENT" = "$OMEGA_REPO_ROOT" ]; then
      echo "lattice paths: cannot find repository root from $OMEGA_GATE_DIR" >&2
      exit 2
    fi
    OMEGA_REPO_ROOT=$OMEGA_PATH_PARENT
  done
  unset OMEGA_PATH_PARENT
fi
. "$OMEGA_REPO_ROOT/tools/lattice/paths.sh" || exit $?
cd "$OMEGA_GATE_DIR"
command -v python3 >/dev/null 2>&1 || { echo "io exhaust: skipped (python3 absent)"; exit 0; }
. "${OMEGA_PATH_BETA_COMPILER}"/artifact_env.sh
SEED="${OMEGA_PATH_ALPHA}"/$ALPHA_SEED
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
BC="$T/bc.exe"
stamp_beta_compiler "$BC" >/dev/null 2>&1 || { echo "io exhaust: lattice bc artifact unavailable"; exit 1; }
N=${1:-40}
PASS=0; FAIL=0
i=1
while [ "$i" -le "$N" ]; do
  s=$((880000 + i))
  python3 io-fuzz-gen.py "$s" > "$T/p.beta"
  if ! ( "$BC" < "$T/p.beta" > "$T/p.tape" 2>/dev/null \
         && stamp_seed "$T/p.tape" "$SEED" "$T/p.exe" >/dev/null 2>&1 ); then
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : bc/assembler could not build the program"; i=$((i+1)); continue; fi
  if python3 io-verify.py "$T/p.beta" "$T/p.exe"; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  (seed=$s program:)"; sed 's/^/    /' "$T/p.beta"; fi
  i=$((i + 1))
done
echo "exhaustive I/O correctness (interpret == compile+run over ALL 256 input bytes, $N random programs = $((N * 256)) input cases): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
