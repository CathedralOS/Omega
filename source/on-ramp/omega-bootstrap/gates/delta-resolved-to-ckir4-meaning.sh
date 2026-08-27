#!/usr/bin/env sh
# Persisted-Beta/Gamma meaning probe for the shared resolved-source lowerer.
set -eu

MODE=${1:-v4}
GAMMA_CEILING=2300000
case "$MODE" in
  v4) SCHEMA_LABEL=CKIR4; FRAME_COMMAND=pack ;;
  v6) SCHEMA_LABEL=CKIR6; FRAME_COMMAND=pack-v7 ;;
  v7) SCHEMA_LABEL=CKIR7; FRAME_COMMAND=pack-v8 ;;
  v8) SCHEMA_LABEL=CKIR8; FRAME_COMMAND=pack-v9 ;;
  v9) SCHEMA_LABEL=CKIR9; FRAME_COMMAND=pack-v10 ;;
  v10) SCHEMA_LABEL=CKIR10; FRAME_COMMAND=pack-v11 ;;
  v11) SCHEMA_LABEL=CKIR11; FRAME_COMMAND=pack-v12 ;;
  v15) SCHEMA_LABEL=CKIR15; FRAME_COMMAND=pack-v16; GAMMA_CEILING=2800000 ;;
  *) echo "usage: delta-resolved-to-ckir4-meaning.sh [v4|v6|v7|v8|v9|v10|v11|v15]" >&2; exit 2 ;;
esac

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "resolved-to-$SCHEMA_LABEL meaning: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "resolved-to-$SCHEMA_LABEL meaning: skipped (native comparison requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "resolved-to-$SCHEMA_LABEL meaning: skipped ($TOOL absent)"
    exit 0
  }
done

LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir4.alp"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir4-fixture.py"
FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir4-frame.py"
if [ "$MODE" = v4 ]; then
  REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v4_reference.py"
elif [ "$MODE" = v6 ]; then
  REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v6_reference.py"
elif [ "$MODE" = v7 ]; then
  REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v7_reference.py"
elif [ "$MODE" = v8 ]; then
  REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v8_reference.py"
elif [ "$MODE" = v9 ]; then
  REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v9_reference.py"
elif [ "$MODE" = v10 ]; then
  REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v10_reference.py"
elif [ "$MODE" = v15 ]; then
  REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v15_reference.py"
else
  REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v11_reference.py"
fi
RUNNER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-ckir4-meaning-runner.py"
DECODER="$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py"
for REQUIRED in "$LOWERER" "$RESOLVER" "$FIXTURE" "$FRAME" "$REFERENCE" "$RUNNER" "$DECODER"; do
  [ -f "$REQUIRED" ] || { echo "resolved-to-$SCHEMA_LABEL meaning: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
stamp_beta_compiler "$T/bc.exe" >/dev/null || {
  echo "resolved-to-$SCHEMA_LABEL meaning FAIL - Beta compiler artifact" >&2
  exit 1
}
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" || {
  echo "resolved-to-$SCHEMA_LABEL meaning FAIL - omega2gamma build" >&2; exit 1;
}
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || {
  echo "resolved-to-$SCHEMA_LABEL meaning FAIL - Gamma interpreter build" >&2; exit 1;
}

# Translate the lowerer exactly once; all accepted/rejected observations below
# reuse the same persisted-Beta-produced Gamma program.
python3 -B "$RUNNER" elaborate "$T/elaborate.exe" "$LOWERER" \
  "$T/lowerer.gamma" "$T/timings.tsv" "resolved-to-$SCHEMA_LABEL meaning" 40 "$GAMMA_CEILING"

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer.native" >/dev/null

if [ "$MODE" = v4 ]; then
python3 - "$T/semantic.omg" "$T/resource.omg" <<'PY'
from pathlib import Path
import sys

def source(duplicate: bool) -> str:
    final = "f0: 5" if duplicate else "f4: 5"
    return f'''data MeaningFive [copy] {{
    f0: u8; f1: u8; f2: u8; f3: u8; f4: u8;
}}
data MeaningFiveProbe {{ value: MeaningFive; scalar: u8; }}
machine MeaningFiveProbe::run(&mut self) -> u8 {{
    self.scalar = 70;
    self.value = MeaningFive {{ f0: self.scalar, f1: 2, f2: 3, f3: 4, {final} }};
    self.scalar
}}
'''

Path(sys.argv[1]).write_text(source(True), encoding="ascii")
Path(sys.argv[2]).write_text(source(False), encoding="ascii")
PY
elif [ "$MODE" = v6 ]; then
python3 - "$T/canonical.omg" "$T/semantic.omg" "$T/resource.omg" <<'PY'
from pathlib import Path
import sys

canonical = '''data LogicalNotMeaning {}
machine LogicalNotMeaning::run(&mut self) -> u8 {
    transition !!false {
        true -> failed()
        false -> passed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
semantic = canonical.replace("!!false", "!1", 1)
resource = canonical.replace("!!false", "!!!!!!!!false", 1)
for path, source in zip(sys.argv[1:], (canonical, semantic, resource)):
    Path(path).write_text(source, encoding="ascii")
PY
elif [ "$MODE" = v7 ]; then
python3 - "$T/canonical.omg" "$T/semantic.omg" "$T/resource.omg" <<'PY'
from pathlib import Path
import sys

canonical = '''data LogicalBinaryMeaning {}
machine LogicalBinaryMeaning::run(&mut self) -> u8 {
    transition !false || false && true {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
semantic = '''data LogicalBinaryMeaning {}
machine LogicalBinaryMeaning::probe(&self) -> bool { true }
machine LogicalBinaryMeaning::run(&mut self) -> u8 {
    transition true || self.probe() {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
resource = '''data LogicalBinaryMeaning {}
machine LogicalBinaryMeaning::run(&mut self) -> u8 {
    transition !!!!!!!true && true { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
for path, source in zip(sys.argv[1:], (canonical, semantic, resource)):
    Path(path).write_text(source, encoding="ascii")
PY
elif [ "$MODE" = v8 ]; then
python3 - "$T/canonical.omg" "$T/semantic.omg" "$T/resource.omg" <<'PY'
from pathlib import Path
import sys

canonical = '''data ScalarEqualMeaning {}
machine ScalarEqualMeaning::run(&mut self) -> u8 {
    transition 1 < 2 == true && 70 == 70 {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
semantic = '''data ScalarEqualMeaning {}
machine ScalarEqualMeaning::probe(&self) -> u32 { 70 }
machine ScalarEqualMeaning::run(&mut self) -> u8 {
    transition self.probe() == 70 {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
resource = '''data ScalarEqualMeaning {}
machine ScalarEqualMeaning::run(&mut self) -> u8 {
    transition true == true == true == true == true == true == true == true == true {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
for path, source in zip(sys.argv[1:], (canonical, semantic, resource)):
    Path(path).write_text(source, encoding="ascii")
PY
elif [ "$MODE" = v9 ]; then
python3 - "$T/canonical.omg" "$T/semantic.omg" "$T/resource.omg" <<'PY'
from pathlib import Path
import sys

canonical = '''data OrderedGreaterMeaning {}
machine OrderedGreaterMeaning::run(&mut self) -> u8 {
    transition 3 > 2 == true && 2 >= 2 && 70 == 70 {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
semantic = '''data OrderedGreaterMeaning {}
machine OrderedGreaterMeaning::probe(&self) -> u32 { 70 }
machine OrderedGreaterMeaning::run(&mut self) -> u8 {
    transition self.probe() > 69 {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
resource = '''data OrderedGreaterMeaning {}
machine OrderedGreaterMeaning::run(&mut self) -> u8 {
    transition 9 > 8 == true == true == true == true == true == true == true {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
for path, source in zip(sys.argv[1:], (canonical, semantic, resource)):
    Path(path).write_text(source, encoding="ascii")
PY
elif [ "$MODE" = v10 ]; then
python3 - "$T/canonical.omg" "$T/semantic.omg" "$T/resource.omg" <<'PY'
from pathlib import Path
import sys

canonical = '''data IntegerWidenMeaning { byte: u8; wide: u32 in Trapping; }
machine IntegerWidenMeaning::run(&mut self) -> u8 {
    self.byte = 0;
    self.wide = self.byte as u32 in Trapping;
    self.byte = 70;
    self.wide = (self.byte) as u32 in Trapping;
    self.byte = 255;
    self.wide = self.byte as u32 in Trapping;
    transition self.wide == 255 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
semantic = '''data IntegerWidenMeaning { byte: u8; wide: u32 in Trapping; }
machine IntegerWidenMeaning::probe(&self) -> u8 { self.byte }
machine IntegerWidenMeaning::run(&mut self) -> u8 {
    self.wide = self.probe() as u32 in Trapping;
    70
}
'''
resource = '''data IntegerWidenMeaning { byte: u8; wide: u32 in Trapping; }
machine IntegerWidenMeaning::run(&mut self) -> u8 {
    self.wide = self.byte as u32 in Trapping;
    transition !!!!!!!!false {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
for path, source in zip(sys.argv[1:], (canonical, semantic, resource)):
    Path(path).write_text(source, encoding="ascii")
PY
elif [ "$MODE" = v11 ]; then
python3 - "$T/canonical.omg" "$T/semantic.omg" "$T/resource.omg" <<'PY'
from pathlib import Path
import sys

canonical = '''data TrappingAddMeaning { cursor: u32 in Trapping; }
machine TrappingAddMeaning::run(&mut self) -> u8 {
    self.cursor = 69;
    self.cursor = self.cursor + 1;
    transition self.cursor == 70 { true -> passed() false -> failed() }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
semantic = '''data TrappingAddMeaning { cursor: u8; }
machine TrappingAddMeaning::run(&mut self) -> u8 {
    self.cursor = 69;
    self.cursor = self.cursor + 1;
    70
}
'''
resource = '''data TrappingAddMeaning { cursor: u32 in Trapping; }
machine TrappingAddMeaning::run(&mut self) -> u8 {
    self.cursor = self.cursor + 1;
    transition !!!!!!!!false {
        true -> passed()
        false -> failed()
    }
    state passed(&mut self) { 70 }
    state failed(&mut self) { 0 }
}
'''
for path, source in zip(sys.argv[1:], (canonical, semantic, resource)):
    Path(path).write_text(source, encoding="ascii")
PY
else
python3 - "$T/canonical.omg" "$T/semantic.omg" "$T/resource.omg" <<'PY'
from pathlib import Path
import sys

canonical = '''data GuardedViewMeaning { result: u8; }
machine GuardedViewMeaning::run(&mut self) -> u8 {
    transition { _ -> inspect("GF", 11) }
    state inspect(&mut self, view: &[u8], pass: u8) {
        transition view.len > 0 {
            true -> emit(view[0], view[1..], pass)
            false -> finish(pass)
        }
    }
    state emit(&mut self, head: u8, view: &[u8], pass: u8) {
        self.result = head;
        transition view.len > 0 {
            true -> emit(view[0], view[1..], pass)
            false -> finish(pass)
        }
    }
    state finish(&mut self, pass: u8) { self.result }
}
'''
semantic = canonical.replace("false -> finish(pass)", "false -> finish(70)", 1)
resource = canonical
for path, source in zip(sys.argv[1:], (canonical, semantic, resource)):
    Path(path).write_text(source, encoding="ascii")
PY
fi

prepare() { # label owner machine source...
  LABEL=$1 OWNER=$2 MACHINE=$3
  shift 3
  python3 -B "$FIXTURE" build "$T/$LABEL.omgc" "$OWNER" "$MACHINE" "$@"
  "$T/resolver.native" < "$T/$LABEL.omgc" > "$T/$LABEL.omgrsw"
  if [ "$MODE" = v15 ]; then
    python3 - "$T/$LABEL.omgrsw" <<'PY'
from pathlib import Path
import struct, sys
raw = Path(sys.argv[1]).read_bytes()
if len(raw) < 10 or raw[:8] != b"OMGRSW4\0" or struct.unpack_from("<H", raw, 8)[0] != 4:
    raise SystemExit("CKIR15 meaning carrier did not select exact OMGRSW4")
PY
  elif [ "$MODE" = v6 ] || [ "$MODE" = v7 ] || [ "$MODE" = v8 ] || [ "$MODE" = v9 ] || [ "$MODE" = v10 ] || [ "$MODE" = v11 ]; then
    python3 - "$T/$LABEL.omgrsw" <<'PY'
from pathlib import Path
import struct, sys
raw = Path(sys.argv[1]).read_bytes()
if len(raw) < 10 or raw[:8] != b"OMGRSW1\0" or struct.unpack_from("<H", raw, 8)[0] != 1:
    raise SystemExit("Boolean meaning carrier did not select least OMGRSW1")
PY
  fi
  python3 -B "$FRAME" "$FRAME_COMMAND" "$T/$LABEL.omgc" "$T/$LABEL.omgrsw" > "$T/$LABEL.omglow"
}
if [ "$MODE" = v4 ]; then
prepare canonical FieldReceiverProbe run \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir4-runtime-records/direct-field-receiver.omg"
prepare semantic-251 MeaningFiveProbe run "$T/semantic.omg"
prepare resource-252 MeaningFiveProbe run "$T/resource.omg"
elif [ "$MODE" = v6 ]; then
prepare canonical LogicalNotMeaning run "$T/canonical.omg"
prepare semantic-251 LogicalNotMeaning run "$T/semantic.omg"
prepare resource-252 LogicalNotMeaning run "$T/resource.omg"
elif [ "$MODE" = v7 ]; then
prepare canonical LogicalBinaryMeaning run "$T/canonical.omg"
prepare semantic-251 LogicalBinaryMeaning run "$T/semantic.omg"
prepare resource-252 LogicalBinaryMeaning run "$T/resource.omg"
elif [ "$MODE" = v8 ]; then
prepare canonical ScalarEqualMeaning run "$T/canonical.omg"
prepare semantic-251 ScalarEqualMeaning run "$T/semantic.omg"
prepare resource-252 ScalarEqualMeaning run "$T/resource.omg"
elif [ "$MODE" = v9 ]; then
prepare canonical OrderedGreaterMeaning run "$T/canonical.omg"
prepare semantic-251 OrderedGreaterMeaning run "$T/semantic.omg"
prepare resource-252 OrderedGreaterMeaning run "$T/resource.omg"
elif [ "$MODE" = v10 ]; then
prepare canonical IntegerWidenMeaning run "$T/canonical.omg"
prepare semantic-251 IntegerWidenMeaning run "$T/semantic.omg"
prepare resource-252 IntegerWidenMeaning run "$T/resource.omg"
elif [ "$MODE" = v11 ]; then
prepare canonical TrappingAddMeaning run "$T/canonical.omg"
prepare semantic-251 TrappingAddMeaning run "$T/semantic.omg"
prepare resource-252 TrappingAddMeaning run "$T/resource.omg"
else
prepare canonical GuardedViewMeaning run "$T/canonical.omg"
prepare semantic-251 GuardedViewMeaning run "$T/semantic.omg"
prepare resource-252 GuardedViewMeaning run "$T/resource.omg"
# Select the first rejected compilation-component length in the outer frame.
# The lowerer must choose 252 before trusting or reading the mismatched body.
python3 - "$T/resource-252.omglow" <<'PY'
from pathlib import Path
import struct, sys

path = Path(sys.argv[1])
raw = bytearray(path.read_bytes())
struct.pack_into("<I", raw, 20, 267_281)
path.write_bytes(raw)
PY
fi
: > "$T/empty.expected"

native_case() { # label expected-status
  set +e
  "$T/lowerer.native" < "$T/$1.omglow" > "$T/$1.expected"
  STATUS=$?
  set -e
  [ "$STATUS" -eq "$2" ] || {
    echo "resolved-to-$SCHEMA_LABEL meaning FAIL - $1 native status $STATUS, expected $2" >&2
    exit 1
  }
  if [ "$2" -ne 0 ] && [ -s "$T/$1.expected" ]; then
    echo "resolved-to-$SCHEMA_LABEL meaning FAIL - $1 native rejection published bytes" >&2
    exit 1
  fi
}
native_case canonical 0
native_case semantic-251 251
native_case resource-252 252
[ "$(python3 -B "$REFERENCE" run "$T/canonical.expected")" = 70 ] || {
  echo "resolved-to-$SCHEMA_LABEL meaning FAIL - canonical $SCHEMA_LABEL result is not 70" >&2
  exit 1
}
if [ "$MODE" = v4 ]; then
  python3 -B "$FIXTURE" inspect "$T/canonical.expected" | \
    grep "'opcodes': .*13" >/dev/null || {
    echo "resolved-to-CKIR4 meaning FAIL - canonical CKIR4 omitted ConstructRecord" >&2
    exit 1
  }
fi

launch_gamma() { # label timeout
  python3 -B "$RUNNER" run "$T/interp.exe" "$T/lowerer.gamma" \
    "$T/$1.omglow" "$T/$1.observation" "$T/timings.tsv" \
    "resolved-to-$SCHEMA_LABEL meaning $1" "$2"
}
check_gamma() { # label expected-status
  STATUS=$(python3 -B "$DECODER" "$T/$1.observation" "$T/$1.stdout")
  [ "$STATUS" -eq "$2" ] || {
    echo "resolved-to-$SCHEMA_LABEL meaning FAIL - $1 status $STATUS, expected $2" >&2
    exit 1
  }
  cmp "$T/$1.stdout" "$T/$1.expected" >/dev/null || {
    echo "resolved-to-$SCHEMA_LABEL meaning FAIL - $1 publication differs" >&2
    exit 1
  }
}
launch_gamma canonical 180 & CANONICAL_PID=$!
launch_gamma semantic-251 180 & SEMANTIC_PID=$!
launch_gamma resource-252 180 & RESOURCE_PID=$!
set +e
wait "$CANONICAL_PID"; CANONICAL_WAIT=$?
wait "$SEMANTIC_PID"; SEMANTIC_WAIT=$?
wait "$RESOURCE_PID"; RESOURCE_WAIT=$?
set -e
[ "$CANONICAL_WAIT" -eq 0 ] && [ "$SEMANTIC_WAIT" -eq 0 ] && \
  [ "$RESOURCE_WAIT" -eq 0 ] || {
  echo "resolved-to-$SCHEMA_LABEL meaning FAIL - Gamma child status canonical=$CANONICAL_WAIT semantic=$SEMANTIC_WAIT resource=$RESOURCE_WAIT" >&2
  exit 1
}
check_gamma canonical 0
check_gamma semantic-251 251
check_gamma resource-252 252

python3 - "$T/timings.tsv" "$T/canonical.expected" "$MODE" "$SCHEMA_LABEL" <<'PY'
from pathlib import Path
import sys

rows = []
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    seconds, size, label = line.split("\t", 2)
    rows.append(f"{label}={float(seconds):.2f}s/{size}B")
if sys.argv[3] == "v4":
    summary = ("ConstructRecord -> Call -> Copy result 70; malformed-five=251 "
               "before valid-five=252")
elif sys.argv[3] == "v6":
    summary = ("LogicalNot false->true->false result 70; non-bool=251 and "
               "expression-depth-9=252")
elif sys.argv[3] == "v7":
    summary = ("pure LogicalNot/LogicalAnd/LogicalOr result 70 with && precedence; "
               "effectful short-circuit operand=251 and expression-depth-9=252")
elif sys.argv[3] == "v8":
    summary = ("pure same-carrier ScalarEqual result 70 with ordering/equality/logical "
               "precedence; effectful operand=251 and expression-depth-9=252")
elif sys.argv[3] == "v9":
    summary = ("pure same-carrier Greater/GreaterEqual result 70 with authored order and "
               "ordering/equality/logical precedence; effectful operand=251 and "
               "expression-depth-9=252")
elif sys.argv[3] == "v10":
    summary = ("pure exact-u8 IntegerWiden preserves 0/70/255 into canonical u32 Trapping "
               "with result 70; effectful operand=251 and expression-depth-9=252")
elif sys.argv[3] == "v11":
    summary = ("canonical u32-in-Trapping leaf-plus-literal Add produces result 70; "
               "nonselected carrier=251 and expression-depth-9=252")
else:
    summary = ("two guarded shared-view occurrences preserve exact direct pass vectors, "
               "execute recurrent head/tail safely, and return 70; non-binder false vector=251 "
               "and outer component capacity=252")
print(f"resolved-to-{sys.argv[4]} meaning: {summary}; exact publication through "
      "canonical Gamma; " + " ".join(rows)
      + f" {sys.argv[4]}={Path(sys.argv[2]).stat().st_size}B")
PY
