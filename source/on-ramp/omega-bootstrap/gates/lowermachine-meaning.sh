#!/usr/bin/env sh
# Focused lower-rung meaning and capacity evidence for the existing Delta
# lowermachine, including a complete tiny compile through canonical Gamma.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "lowermachine meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

for TOOL in perl python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "lowermachine meaning: $TOOL required" >&2
    exit 2
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "lowermachine meaning: Beta compiler artifact unavailable" >&2
  exit 1
}
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$2" >/dev/null 2>&1
}

build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" || {
  echo "lowermachine meaning: omega2gamma build failed" >&2
  exit 1
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "lowermachine meaning: Gamma interpreter build failed" >&2
  exit 1
}

set +e
perl -e 'alarm 10; exec @ARGV' "$T/elaborate.exe" \
  < "$OMEGA_PATH_DELTA/samples/lowermachine.alp" > "$T/lowermachine.gamma"
ELABORATE_STATUS=$?
set -e
[ "$ELABORATE_STATUS" -eq 0 ] || {
  echo "lowermachine meaning: lowermachine elaboration exited $ELABORATE_STATUS" >&2
  exit 1
}
[ -s "$T/lowermachine.gamma" ] && ! grep -q 'E2G-UNSUPPORTED' "$T/lowermachine.gamma" || {
  echo "lowermachine meaning: lowermachine was explicitly unsupported" >&2
  exit 1
}
LOWERMACHINE_GAMMA_BYTES=$(wc -c < "$T/lowermachine.gamma" | tr -d ' ')
[ "$LOWERMACHINE_GAMMA_BYTES" -le 393216 ] || {
  echo "lowermachine meaning: Gamma output is $LOWERMACHINE_GAMMA_BYTES bytes" >&2
  exit 1
}

python3 - "$T/lowermachine.gamma" "$OMEGA_PATH_DELTA/samples/arith.alp" \
  "$T/lowermachine-arith.gamma" <<'PY'
import pathlib
import sys

program = pathlib.Path(sys.argv[1]).read_text(encoding="ascii")
source = pathlib.Path(sys.argv[2]).read_bytes()
stdin = "Nil"
for byte in reversed(source):
    stdin = f"(Cons {byte} {stdin})"
if program.count("STDIN") != 1:
    raise SystemExit("lowermachine meaning: expected one STDIN placeholder")
pathlib.Path(sys.argv[3]).write_text(program.replace("STDIN", stdin), encoding="ascii")
PY

set +e
perl -e 'alarm 60; exec @ARGV' "$T/interp.exe" \
  < "$T/lowermachine-arith.gamma" > "$T/lowermachine-arith.observation"
LOWERMACHINE_INTERP_STATUS=$?
set -e
[ "$LOWERMACHINE_INTERP_STATUS" -eq 0 ] || {
  echo "lowermachine meaning: Gamma execution exited $LOWERMACHINE_INTERP_STATUS" >&2
  exit 1
}
LOWERMACHINE_GAMMA_STATUS=$(python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py" \
  "$T/lowermachine-arith.observation" "$T/lowermachine-arith.gamma.stdout")
[ "$LOWERMACHINE_GAMMA_STATUS" -eq 0 ] || {
  echo "lowermachine meaning: Gamma compiler status $LOWERMACHINE_GAMMA_STATUS" >&2
  exit 1
}
python3 - "$T/lowermachine-arith.gamma.stdout" <<'PY'
import hashlib
import pathlib
import sys

artifact = pathlib.Path(sys.argv[1]).read_bytes()
assert len(artifact) == 800
assert hashlib.sha256(artifact).hexdigest() == (
    "e66dd3be044a7003df1d84f3e6497309b881b0ebcbcfe002343bdedf7d0caa88"
)
PY

write_state_source() {
  COUNT=$1
  OUTPUT=$2
  python3 - "$COUNT" "$OUTPUT" <<'PY'
import pathlib
import sys

count = int(sys.argv[1])
states = "".join(
    f"state s{index}(){{self.console.exit_process(0)}}" for index in range(count)
)
source = (
    "boundary trait Console{machine exit_process(return_code:i32);}"
    "data Main{console:Console;}"
    "machine Main::main(&mut self){transition 0{_->s0()}" + states + "}"
)
pathlib.Path(sys.argv[2]).write_text(source, encoding="ascii")
PY
}

write_state_source 1024 "$T/states-1024.alp"
set +e
perl -e 'alarm 10; exec @ARGV' "$T/elaborate.exe" \
  < "$T/states-1024.alp" > "$T/states-1024.gamma"
STATES_1024_STATUS=$?
set -e
[ "$STATES_1024_STATUS" -eq 0 ] \
  && [ -s "$T/states-1024.gamma" ] \
  && ! grep -q 'E2G-UNSUPPORTED' "$T/states-1024.gamma" || {
    echo "lowermachine meaning: 1024-state boundary was not admitted" >&2
    exit 1
  }

write_state_source 1025 "$T/states-1025.alp"
set +e
perl -e 'alarm 10; exec @ARGV' "$T/elaborate.exe" \
  < "$T/states-1025.alp" > "$T/states-1025.gamma"
STATES_1025_STATUS=$?
set -e
[ "$STATES_1025_STATUS" -eq 0 ] \
  && grep -q 'E2G-UNSUPPORTED-machine-state-capacity' "$T/states-1025.gamma" || {
    echo "lowermachine meaning: 1025 states did not refuse explicitly" >&2
    exit 1
  }

python3 - "$T/state-params-5.alp" <<'PY'
import pathlib
import sys

pathlib.Path(sys.argv[1]).write_text(
    "boundary trait Console{machine exit_process(return_code:i32);}"
    "data Main{console:Console;}"
    "machine Main::main(&mut self){"
    "transition 0{_->s0(0,0,0,0,0)}"
    "state s0(a:i32,b:i32,c:i32,d:i32,e:i32){self.console.exit_process(0)}"
    "}",
    encoding="ascii",
)
PY
"$T/elaborate.exe" < "$T/state-params-5.alp" > "$T/state-params-5.gamma"
grep -q 'E2G-UNSUPPORTED-state-parameter-capacity' "$T/state-params-5.gamma" || {
  echo "lowermachine meaning: five state parameters did not refuse explicitly" >&2
  exit 1
}

python3 - "$T/tree-array.alp" <<'PY'
import pathlib
import sys

pathlib.Path(sys.argv[1]).write_text(
    "boundary trait Console {"
    "machine write_byte(b: i32);"
    "machine exit_process(return_code: i32);"
    "}"
    "data Main { console: Console; buf: [i32; 1024]; }"
    "machine Main::main(&mut self) {"
    "self.buf[0] = 7;"
    "self.buf[511] = 11;"
    "self.buf[1023] = 13;"
    "self.buf[511] = 17;"
    "self.console.write_byte(self.buf[0]);"
    "self.console.write_byte(self.buf[511]);"
    "self.console.write_byte(self.buf[1023]);"
    "self.console.exit_process(self.buf[0] + self.buf[511] + self.buf[1023] + 5);"
    "}",
    encoding="ascii",
)
PY

"$T/elaborate.exe" < "$T/tree-array.alp" > "$T/tree-array.gamma"
[ -s "$T/tree-array.gamma" ] && ! grep -q 'E2G-UNSUPPORTED' "$T/tree-array.gamma" || {
  echo "lowermachine meaning: 1024-cell tree array unsupported" >&2
  exit 1
}
set +e
perl -e 'alarm 20; exec @ARGV' "$T/interp.exe" \
  < "$T/tree-array.gamma" > "$T/tree-array.observation"
TREE_INTERP_STATUS=$?
set -e
[ "$TREE_INTERP_STATUS" -eq 0 ] || {
  echo "lowermachine meaning: tree-array Gamma execution exited $TREE_INTERP_STATUS" >&2
  exit 1
}
TREE_STATUS=$(python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py" \
  "$T/tree-array.observation" "$T/tree-array.gamma.stdout")
[ "$TREE_STATUS" -eq 42 ] || {
  echo "lowermachine meaning: tree-array Gamma status $TREE_STATUS" >&2
  exit 1
}
python3 - "$T/tree-array.gamma.stdout" <<'PY'
import pathlib
import sys

assert pathlib.Path(sys.argv[1]).read_bytes() == bytes((7, 17, 13))
PY

python3 - "$T/tree-too-large.alp" <<'PY'
import pathlib
import sys

pathlib.Path(sys.argv[1]).write_text(
    "boundary trait Console{machine exit_process(return_code:i32);}"
    "data Main{console:Console;buf:[i32;524289];}"
    "machine Main::main(&mut self){self.console.exit_process(0)}",
    encoding="ascii",
)
PY
"$T/elaborate.exe" < "$T/tree-too-large.alp" > "$T/tree-too-large.gamma"
grep -q 'E2G-UNSUPPORTED-array-capacity' "$T/tree-too-large.gamma" || {
  echo "lowermachine meaning: oversized tree did not refuse explicitly" >&2
  exit 1
}

# Native comparison belonged to the retired external Delta producer. This gate
# now states only lower-rooted Gamma meaning and resource facts. Native and
# self-built comparisons return once the canonical Delta artifact is published.

python3 - "$T/source-exact.gamma" "$T/source-plus-one.gamma" <<'PY'
import pathlib
import sys

base = b"(+ 40 2)"
exact = base + b" " * (4 * 1024 * 1024 - len(base))
assert len(exact) == 4 * 1024 * 1024
pathlib.Path(sys.argv[1]).write_bytes(exact)
pathlib.Path(sys.argv[2]).write_bytes(exact + b" ")
PY

set +e
perl -e 'alarm 10; exec @ARGV' "$T/interp.exe" \
  < "$T/source-exact.gamma" > "$T/source-exact.out"
SOURCE_EXACT_STATUS=$?
perl -e 'alarm 10; exec @ARGV' "$T/interp.exe" \
  < "$T/source-plus-one.gamma" > "$T/source-plus-one.out"
SOURCE_PLUS_ONE_STATUS=$?
set -e
[ "$SOURCE_EXACT_STATUS" -eq 42 ] \
  && [ "$(tr -d '\n' < "$T/source-exact.out")" = 42 ] || {
    echo "lowermachine meaning: exact 4 MiB Gamma source was not admitted" >&2
    exit 1
  }
[ "$SOURCE_PLUS_ONE_STATUS" -eq 252 ] && [ ! -s "$T/source-plus-one.out" ] || {
  echo "lowermachine meaning: 4 MiB+1 Gamma source did not exhaust cleanly" >&2
  exit 1
}

python3 - "$T/args-exact.gamma" "$T/args-plus-one.gamma" <<'PY'
import pathlib
import sys

def program(count: int) -> str:
    params = " ".join(f"p{index}" for index in range(count))
    return f"(def f ({params}) 42) (f " + "0 " * count + ")"

pathlib.Path(sys.argv[1]).write_text(program(512), encoding="ascii")
pathlib.Path(sys.argv[2]).write_text(program(513), encoding="ascii")
PY

set +e
perl -e 'alarm 10; exec @ARGV' "$T/interp.exe" \
  < "$T/args-exact.gamma" > "$T/args-exact.out"
ARGS_EXACT_STATUS=$?
perl -e 'alarm 10; exec @ARGV' "$T/interp.exe" \
  < "$T/args-plus-one.gamma" > "$T/args-plus-one.out"
ARGS_PLUS_ONE_STATUS=$?
set -e
[ "$ARGS_EXACT_STATUS" -eq 42 ] \
  && [ "$(tr -d '\n' < "$T/args-exact.out")" = 42 ] || {
    echo "lowermachine meaning: exact 512-value argument scratch was not admitted" >&2
    exit 1
  }
[ "$ARGS_PLUS_ONE_STATUS" -eq 253 ] && [ ! -s "$T/args-plus-one.out" ] || {
  echo "lowermachine meaning: 513 argument values did not exhaust cleanly" >&2
  exit 1
}

echo "lowermachine meaning: $LOWERMACHINE_GAMMA_BYTES-byte marker-free elaboration, exact arith compile, tree behavior, and state/source/argument capacity teeth passed"
