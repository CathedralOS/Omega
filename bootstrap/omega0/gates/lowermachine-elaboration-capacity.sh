#!/usr/bin/env sh
# Focused lower-rung capacity evidence for the existing Delta lowermachine.
# This gate proves elaboration and the large-array representation; it does not
# claim that the complete lowermachine executes within Gamma's current arena.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "lowermachine elaboration capacity: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

for TOOL in perl python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "lowermachine elaboration capacity: $TOOL required" >&2
    exit 2
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "lowermachine elaboration capacity: Beta compiler artifact unavailable" >&2
  exit 1
}
ASM="$OMEGA_PATH_BETA_ASSEMBLER/$BETA_SEED"
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$2" >/dev/null 2>&1
}

build_beta "$OMEGA_PATH_OMEGA0/meaning/omega2gamma.beta" "$T/elaborate.exe" || {
  echo "lowermachine elaboration capacity: omega2gamma build failed" >&2
  exit 1
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "lowermachine elaboration capacity: Gamma interpreter build failed" >&2
  exit 1
}

set +e
perl -e 'alarm 10; exec @ARGV' "$T/elaborate.exe" \
  < "$OMEGA_PATH_DELTA/samples/lowermachine.alp" > "$T/lowermachine.gamma"
ELABORATE_STATUS=$?
set -e
[ "$ELABORATE_STATUS" -eq 0 ] || {
  echo "lowermachine elaboration capacity: lowermachine elaboration exited $ELABORATE_STATUS" >&2
  exit 1
}
[ -s "$T/lowermachine.gamma" ] && ! grep -q 'E2G-UNSUPPORTED' "$T/lowermachine.gamma" || {
  echo "lowermachine elaboration capacity: lowermachine was explicitly unsupported" >&2
  exit 1
}
LOWERMACHINE_GAMMA_BYTES=$(wc -c < "$T/lowermachine.gamma" | tr -d ' ')
[ "$LOWERMACHINE_GAMMA_BYTES" -le 262144 ] || {
  echo "lowermachine elaboration capacity: Gamma output is $LOWERMACHINE_GAMMA_BYTES bytes" >&2
  exit 1
}

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
    echo "lowermachine elaboration capacity: 1024-state boundary was not admitted" >&2
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
    echo "lowermachine elaboration capacity: 1025 states did not refuse explicitly" >&2
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
  echo "lowermachine elaboration capacity: five state parameters did not refuse explicitly" >&2
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
  echo "lowermachine elaboration capacity: 1024-cell tree array unsupported" >&2
  exit 1
}
set +e
perl -e 'alarm 20; exec @ARGV' "$T/interp.exe" \
  < "$T/tree-array.gamma" > "$T/tree-array.observation"
TREE_INTERP_STATUS=$?
set -e
[ "$TREE_INTERP_STATUS" -eq 0 ] || {
  echo "lowermachine elaboration capacity: tree-array Gamma execution exited $TREE_INTERP_STATUS" >&2
  exit 1
}
TREE_STATUS=$(python3 "$OMEGA_PATH_OMEGA0/meaning/decode-gamma-output.py" \
  "$T/tree-array.observation" "$T/tree-array.gamma.stdout")
[ "$TREE_STATUS" -eq 42 ] || {
  echo "lowermachine elaboration capacity: tree-array Gamma status $TREE_STATUS" >&2
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
  echo "lowermachine elaboration capacity: oversized tree did not refuse explicitly" >&2
  exit 1
}

case "$(uname -sm)" in
  "Darwin arm64")
    for TOOL in cargo clang codesign; do
      command -v "$TOOL" >/dev/null 2>&1 || {
        echo "lowermachine elaboration capacity: skipped native tree differential ($TOOL absent)"
        TOOL_MISSING=1
      }
    done
    if [ "${TOOL_MISSING:-0}" -eq 0 ]; then
      cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
      DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
        "$T/tree-array.alp" "$T/tree-array.native" >/dev/null
      set +e
      "$T/tree-array.native" > "$T/tree-array.native.stdout"
      TREE_NATIVE_STATUS=$?
      set -e
      [ "$TREE_NATIVE_STATUS" -eq "$TREE_STATUS" ] \
        && cmp "$T/tree-array.native.stdout" "$T/tree-array.gamma.stdout" || {
          echo "lowermachine elaboration capacity: native/Gamma tree-array observation differs" >&2
          exit 1
        }
    fi
    ;;
  *) echo "lowermachine elaboration capacity: native tree differential skipped on $(uname -sm)" ;;
esac

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
    echo "lowermachine elaboration capacity: exact 4 MiB Gamma source was not admitted" >&2
    exit 1
  }
[ "$SOURCE_PLUS_ONE_STATUS" -eq 252 ] && [ ! -s "$T/source-plus-one.out" ] || {
  echo "lowermachine elaboration capacity: 4 MiB+1 Gamma source did not exhaust cleanly" >&2
  exit 1
}

echo "lowermachine elaboration capacity: $LOWERMACHINE_GAMMA_BYTES-byte marker-free elaboration, state/table capacity teeth, tree-array differential, and Gamma source bounds passed"
