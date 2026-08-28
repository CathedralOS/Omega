#!/usr/bin/env sh
# ASSEMBLER DIAMOND — the independent reference assembler (asm_ref.py) agrees with the real one.
#
# asm_ref.py is an independent realization (Python, written from the encoding,
# not ported from assembler.alpha). This gate compares both assemblers over a
# corpus to catch encoding regressions. Agreement is diagnostic evidence, not
# source-to-artifact authority; asm_ref.py is untrusted and runtime never runs it.
set -e
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
command -v python3 >/dev/null 2>&1 || { echo "asm-diamond SKIP — no python3"; exit 0; }
. "${OMEGA_PATH_BETA_COMPILER}/artifact_env.sh"
ASM="./$BETA_SEED"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
BC="$T/bc.exe"
stamp_beta_compiler "$BC" >/dev/null 2>&1 || { echo "assembler diamond: lattice bc artifact unavailable"; exit 1; }
PASS=0; FAIL=0

cmp_asm() {  # name  asmfile
  "$ASM" < "$2" > "$T/real.tape" 2>/dev/null
  if ! python3 asm_ref.py < "$2" > "$T/ref.tape" 2>"$T/e"; then
    FAIL=$((FAIL+1)); echo "  FAIL $1 : asm_ref.py error: $(cat "$T/e")"; return; fi
  if cmp -s "$T/real.tape" "$T/ref.tape"; then PASS=$((PASS+1)); else
    FAIL=$((FAIL+1)); echo "  FAIL $1 : tapes DIFFER (real=$(wc -c <"$T/real.tape"|tr -d ' ') ref=$(wc -c <"$T/ref.tape"|tr -d ' '))"; fi
}

# the assembler's own source (the ultimate case: assemble the real assembler both ways) + its examples
cmp_asm "assembler.alpha (self)" "assembler.alpha"
for ex in examples/*.alpha; do [ -f "$ex" ] && cmp_asm "example $(basename "$ex")" "$ex"; done

# real bc-compiled programs — exercise every opcode + labels + comparisons + memory + I/O + db strings
if [ -x "$BC" ]; then
  gen() { printf '%s\n' "$2" | "$BC" > "$T/$1.asm" 2>/dev/null && cmp_asm "bc: $1" "$T/$1.asm"; }
  gen fact 'proc fact(n){ state c{ to r when (n>1) return 1 } state r{ return n*fact(n-1) } } proc main(){ return fact(5) }'
  gen echo 'proc main(){ let c=read_byte() state l{ to b when (c>=0) return 0 } state b{ write_byte(c) c=read_byte() to l } }'
  gen strs 'proc main(){ emit("Hi!\n") return 0 }'
  gen cmps 'proc main(){ let a=5 return (a<8)*7 + (a>8) + (a==5) }'
  gen mem  'proc main(){ let b=2097152 word[b]=42 return word[b] }'
  # the big one: the checker (assemble bc''s compilation of check.beta both ways)
  if [ -f "${OMEGA_PATH_PROOF_KERNEL}"/implementations/beta/check.beta ] && "$BC" < "${OMEGA_PATH_PROOF_KERNEL}"/implementations/beta/check.beta > "$T/check.asm" 2>/dev/null; then
    cmp_asm "bc: check.beta (the trust anchor)" "$T/check.asm"; fi
else
  echo "  (skipped bc-compiled cases — bc not available)"
fi

echo "assembler diamond (independent reference asm_ref.py assembles byte-identically to assembler.alpha): $PASS ok, $FAIL failed"
[ "$FAIL" = 0 ] || exit 1
