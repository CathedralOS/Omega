#!/usr/bin/env sh
# EXHAUSTIVE I/O CORRECTNESS — for input-reading programs, verify bc compiles correctly over the WHOLE
# bounded input domain, not just sampled inputs. For each random single-byte-reading program (io-fuzz-gen.py),
# compile it once with bc and run it against the reference interpreter for EVERY input byte 0..255
# (io-verify.py), requiring the same exit code and stdout on all 256. This is a complete correctness check
# for single-byte-input programs — the fuzz's "for all inputs" analogue — and it exercises the read_byte /
# write_byte / input-dependent-branch codegen that the fixed-input correctness fuzz does not.
# Deterministic (fixed base seed). Needs python3 + the bc/assembler build; skips cleanly otherwise.
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo "io exhaust: skipped (python3 absent)"; exit 0; }
command -v cargo   >/dev/null 2>&1 || { echo "io exhaust: skipped (no cargo for the on-ramp)"; exit 0; }
. ../alpha/seed_env.sh
SEED=../alpha/$ALPHA_SEED
ASM=../beta/$BETA_SEED
BC=../beta-lang-rs/build/bc.exe
( cd ../beta-lang-rs && sh build.sh ../beta-lang/bc.beta >/dev/null 2>&1 ) || { echo "io exhaust: bc build failed"; exit 1; }
[ -x "$BC" ] && [ -x "$ASM" ] || { echo "io exhaust: skipped (bc/assembler missing)"; exit 0; }

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
N=${1:-40}
PASS=0; FAIL=0
i=1
while [ "$i" -le "$N" ]; do
  s=$((880000 + i))
  python3 io-fuzz-gen.py "$s" > "$T/p.beta"
  if ! ( "$BC" < "$T/p.beta" > "$T/p.asm" 2>/dev/null && "$ASM" < "$T/p.asm" > "$T/p.tape" 2>/dev/null \
         && stamp_seed "$T/p.tape" "$SEED" "$T/p.exe" >/dev/null 2>&1 ); then
    FAIL=$((FAIL+1)); echo "  FAIL seed=$s : bc/assembler could not build the program"; i=$((i+1)); continue; fi
  if python3 io-verify.py "$T/p.beta" "$T/p.exe"; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  (seed=$s program:)"; sed 's/^/    /' "$T/p.beta"; fi
  i=$((i + 1))
done
echo "exhaustive I/O correctness (interpret == compile+run over ALL 256 input bytes, $N random programs = $((N * 256)) input cases): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
