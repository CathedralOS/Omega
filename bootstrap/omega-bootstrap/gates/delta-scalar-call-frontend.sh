#!/usr/bin/env sh
# Focused native gate for the ordinary-source half of the bounded scalar
# in-module call/return conformance slice.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] \
    || { echo "Delta scalar frontend: repository root not found" >&2; exit 2; }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "Delta scalar frontend: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 \
    || { echo "Delta scalar frontend: skipped ($TOOL absent)"; exit 0; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
FRONTEND="$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega-bootstrap-frontend.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP/gates/fixtures/omega-bootstrap-scalar-call-v28.hex"
PRODUCT_CASE_DIR="$T/product-scalar-cases"
mkdir -p "$PRODUCT_CASE_DIR"

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$FRONTEND" "$T/frontend" >/dev/null

python3 - "$FIXTURE" "$T/reference.terminal" <<'PY'
import pathlib
import sys
pathlib.Path(sys.argv[2]).write_bytes(
    bytes.fromhex(pathlib.Path(sys.argv[1]).read_text(encoding="ascii"))
)
PY

bundle_file() {
  SOURCE=$1
  OUTPUT=$2
  python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega_bootstrap_bundle.py" \
    pack main.omg="$SOURCE" > "$OUTPUT"
}

run_file() {
  LABEL=$1
  SOURCE=$2
  EXPECTED=$3
  bundle_file "$SOURCE" "$T/case.bundle"
  set +e
  "$T/frontend" < "$T/case.bundle" > "$T/case.terminal"
  GOT=$?
  set -e
  [ "$GOT" -eq "$EXPECTED" ] || {
    echo "Delta scalar frontend FAIL — $LABEL status $GOT, expected $EXPECTED" >&2
    exit 1
  }
  if [ "$EXPECTED" -eq 0 ]; then
    [ -s "$T/case.terminal" ] || {
      echo "Delta scalar frontend FAIL — $LABEL published no terminal" >&2
      exit 1
    }
  else
    [ ! -s "$T/case.terminal" ] || {
      echo "Delta scalar frontend FAIL — $LABEL published bytes on rejection" >&2
      exit 1
    }
  fi
}

run_text() {
  LABEL=$1
  TEXT=$2
  EXPECTED=$3
  printf '%s' "$TEXT" > "$T/source.omg"
  run_file "$LABEL" "$T/source.omg" "$EXPECTED"
}

CANONICAL='machine caller() -> i32 { return passthrough(73); } machine passthrough(value: i32) -> i32 { return value; }'
run_text "canonical source" "$CANONICAL" 0
cmp "$T/case.terminal" "$T/reference.terminal" >/dev/null || {
  echo "Delta scalar frontend FAIL — canonical source differs from product fixture" >&2
  exit 1
}
cp "$T/case.terminal" "$T/canonical.terminal"

run_text "declaration permutation" \
  'machine passthrough(value:i32)->i32{return value;} machine caller()->i32{return passthrough(73);}' 0
cmp "$T/case.terminal" "$T/canonical.terminal" >/dev/null || {
  echo "Delta scalar frontend FAIL — declaration permutation changed bytes" >&2
  exit 1
}

run_text "arbitrary renamed machines" \
  'machine alpha(x:i32)->i32{return x;} machine zeta()->i32{return alpha(73);}' 0
cmp "$T/case.terminal" "$T/canonical.terminal" >/dev/null || {
  echo "Delta scalar frontend FAIL — two-machine renaming changed bytes" >&2
  exit 1
}
cp "$T/case.terminal" "$PRODUCT_CASE_DIR/renamed-permuted.terminal"

run_text "nested forward calls" \
  'machine root()->i32{return outer(middle(inner(7)));} machine outer(x:i32)->i32{return x;} machine middle(y:i32)->i32{return y;} machine inner(z:i32)->i32{return z;}' 0
cp "$T/case.terminal" "$PRODUCT_CASE_DIR/nested-three-hop.terminal"
run_text "four scalar arguments" \
  'machine root()->i32{return fourth(1,2,3,4);} machine fourth(a:i32,b:i32,c:i32,d:i32)->i32{return d;}' 0
cp "$T/case.terminal" "$PRODUCT_CASE_DIR/four-arguments.terminal"

run_text "signed i32 minimum" 'machine root()->i32{return -2147483648;}' 0
cp "$T/case.terminal" "$T/min.terminal"
cp "$T/case.terminal" "$PRODUCT_CASE_DIR/signed-minimum.terminal"
run_text "signed i32 maximum" 'machine root()->i32{return 2147483647;}' 0
cp "$T/case.terminal" "$T/max.terminal"
cp "$T/case.terminal" "$PRODUCT_CASE_DIR/signed-maximum.terminal"
python3 - "$T/min.terminal" "$T/max.terminal" <<'PY'
import pathlib
import sys
minimum = pathlib.Path(sys.argv[1]).read_bytes()
maximum = pathlib.Path(sys.argv[2]).read_bytes()
if (-(1 << 31)).to_bytes(16, "little", signed=True) not in minimum:
    raise SystemExit("Delta scalar frontend FAIL — i32 minimum payload not retained")
if ((1 << 31) - 1).to_bytes(16, "little", signed=True) not in maximum:
    raise SystemExit("Delta scalar frontend FAIL — i32 maximum payload not retained")
PY

# The complete bundle is validated first; trivia-only source units do not alter
# the one program unit's artifact and cannot participate in its tokens.
: > "$T/empty.omg"
printf '/* auxiliary /* nested */ tail */' > "$T/comment.omg"
printf '%s' "$CANONICAL" > "$T/canonical.omg"
python3 "$OMEGA_PATH_OMEGA_BOOTSTRAP/compiler/omega_bootstrap_bundle.py" pack \
  a/empty.omg="$T/empty.omg" m/program.omg="$T/canonical.omg" \
  z/comment.omg="$T/comment.omg" > "$T/auxiliary.bundle"
"$T/frontend" < "$T/auxiliary.bundle" > "$T/auxiliary.terminal"
cmp "$T/auxiliary.terminal" "$T/canonical.terminal" >/dev/null || {
  echo "Delta scalar frontend FAIL — auxiliary trivia changed terminal" >&2
  exit 1
}

# Malformed/out-of-profile source: semantic rejection 251, no publication.
run_text "duplicate machine" \
  'machine root()->i32{return 0;} machine root()->i32{return 0;}' 251
run_text "duplicate parameter" \
  'machine root()->i32{return f(1,2);} machine f(x:i32,x:i32)->i32{return x;}' 251
run_text "unknown parameter" 'machine root()->i32{return missing;}' 251
run_text "unknown callee" 'machine root()->i32{return missing(1);}' 251
run_text "wrong arity" \
  'machine root()->i32{return f();} machine f(x:i32)->i32{return x;}' 251
run_text "wrong parameter type" \
  'machine root()->i32{return f(1);} machine f(x:u32)->i32{return x;}' 251
run_text "wrong result type" 'machine root()->bool{return 1;}' 251
run_text "positive i32 overflow" 'machine root()->i32{return 2147483648;}' 251
run_text "negative i32 overflow" 'machine root()->i32{return -2147483649;}' 251
run_text "parameterized root" 'machine root(x:i32)->i32{return x;}' 251
run_text "ambiguous roots" \
  'machine left()->i32{return 1;} machine right()->i32{return 2;}' 251
run_text "direct cycle" 'machine loop()->i32{return loop();}' 251
run_text "mutual cycle" \
  'machine left()->i32{return right();} machine right()->i32{return left();}' 251
run_text "unsupported arithmetic" 'machine root()->i32{return 1+2;}' 251
run_text "unsupported extra statement" \
  'machine root()->i32{let x:i32=1;return x;}' 251

# Checked table exhaustion 252, with exact adjacent admissions.
make_chain() {
  COUNT=$1
  OUTPUT=$2
  : > "$OUTPUT"
  INDEX=0
  while [ "$INDEX" -lt "$COUNT" ]; do
    NEXT=$((INDEX + 1))
    if [ "$NEXT" -lt "$COUNT" ]; then
      printf 'machine m%d()->i32{return m%d();}' "$INDEX" "$NEXT" >> "$OUTPUT"
    else
      printf 'machine m%d()->i32{return 0;}' "$INDEX" >> "$OUTPUT"
    fi
    INDEX=$NEXT
  done
}
make_chain 16 "$T/machines-16.omg"
run_file "exact 16-machine ceiling" "$T/machines-16.omg" 0
make_chain 17 "$T/machines-17.omg"
run_file "17th machine" "$T/machines-17.omg" 252

run_text "fifth parameter" \
  'machine root()->i32{return 0;} machine f(a:i32,b:i32,c:i32,d:i32,e:i32)->i32{return e;}' 252
run_text "fifth call argument" \
  'machine root()->i32{return f(1,2,3,4,5);} machine f(a:i32,b:i32,c:i32,d:i32)->i32{return d;}' 252

make_nested() {
  COUNT=$1
  OUTPUT=$2
  printf 'machine root()->i32{return ' > "$OUTPUT"
  INDEX=0
  while [ "$INDEX" -lt "$COUNT" ]; do printf 'id(' >> "$OUTPUT"; INDEX=$((INDEX + 1)); done
  printf 1 >> "$OUTPUT"
  INDEX=0
  while [ "$INDEX" -lt "$COUNT" ]; do printf ')' >> "$OUTPUT"; INDEX=$((INDEX + 1)); done
  printf ';}machine id(x:i32)->i32{return x;}' >> "$OUTPUT"
}
make_nested 15 "$T/operations-16.omg"
run_file "exact 16-operation ceiling" "$T/operations-16.omg" 0
make_nested 16 "$T/operations-17.omg"
run_file "17th operation" "$T/operations-17.omg" 252
make_nested 17 "$T/depth-17.omg"
run_file "17th expression frame" "$T/depth-17.omg" 252

OMEGA_BOOTSTRAP_SCALAR_CALL_CASE_DIR="$PRODUCT_CASE_DIR" \
  cargo test -q -p omega-native-differential-test \
    --test terminal_psi_calls \
    frontend_generated_scalar_terminals_are_product_valid -- --exact

echo "Delta scalar frontend: exact fixture, permutations, signed i32, graph semantics, product validation, and checked ceilings passed"
