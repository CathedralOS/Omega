#!/usr/bin/env sh
# One exact greatest source-realizable OMGLOW3 frame and its one-byte adjacent
# resource exhaustion. This is deliberately not a broad resource matrix.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
    echo "greatest OMGLOW3: repository root not found" >&2
    exit 2
  }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "greatest OMGLOW3: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign rg cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "greatest OMGLOW3: skipped ($TOOL absent)"
    exit 0
  }
done

GENERATOR="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-greatest-frame.py"
FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-frame.py"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v3_reference.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir3.alp"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
DELTA_MANIFEST="$OMEGA_PATH_DELTA_RUST/Cargo.toml"
for REQUIRED in "$GENERATOR" "$FRAME" "$REFERENCE" "$RESOLVER" "$LOWERER" \
  "$LOWERMACHINE" "$DELTA_MANIFEST"; do
  [ -f "$REQUIRED" ] || {
    echo "greatest OMGLOW3: missing $REQUIRED" >&2
    exit 1
  }
done

RESOLVER_PROCEDURES=$(rg -c '^machine ' "$RESOLVER")
LOWERER_PROCEDURES=$(rg -c '^machine ' "$LOWERER")
LOWERMACHINE_PROCEDURES=$(rg -c '^machine ' "$LOWERMACHINE")
for COUNT in "$RESOLVER_PROCEDURES" "$LOWERER_PROCEDURES" "$LOWERMACHINE_PROCEDURES"; do
  [ "$COUNT" -le 128 ] || {
    echo "greatest OMGLOW3: Delta procedure ceiling exceeded ($COUNT/128)" >&2
    exit 1
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"

observe() { # timeout input output expected label command...
  OBS_TIMEOUT=$1 OBS_INPUT=$2 OBS_OUTPUT=$3 OBS_EXPECTED=$4 OBS_LABEL=$5
  shift 5
  python3 -B "$GENERATOR" observe "$OBS_TIMEOUT" "$OBS_INPUT" "$OBS_OUTPUT" \
    "$OBS_EXPECTED" "$T/timings.tsv" "$OBS_LABEL" -- "$@"
}

echo "greatest OMGLOW3: START construction and native/self build" >&2
observe 120 - - 0 cargo-build cargo build -q --manifest-path "$DELTA_MANIFEST"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
export DELTA_ARCH=aarch64
observe 60 - - 0 compile-lowermachine "$DELTA" "$LOWERMACHINE" "$T/lowermachine"
observe 60 - - 0 compile-resolver "$DELTA" "$RESOLVER" "$T/resolver"
observe 60 - - 0 compile-lowerer "$DELTA" "$LOWERER" "$T/lowerer.native"
observe 60 "$LOWERER" "$T/lowerer.self.s" 0 self-source "$T/lowermachine"
observe 30 - - 0 clang-self clang -arch arm64 -o "$T/lowerer.self" "$T/lowerer.self.s"
observe 30 - - 0 codesign-self codesign -f -s - "$T/lowerer.self"

observe 10 - - 0 generate python3 -B "$GENERATOR" build "$T/fixture"
EXACT_COMP="$T/fixture/exact.omgc"
ADJACENT_COMP="$T/fixture/adjacent.omgc"

# Resolve exactly once. The adjacent source differs only by one trailing space
# and deliberately reuses the positive witness. The added source space exceeds
# both aggregate-source and nested-bundle ceilings; canonical decoder order
# selects the public nested-bundle extent diagnostic first.
observe 30 "$EXACT_COMP" "$T/exact.omgrsw1" 0 resolver "$T/resolver"
observe 10 - - 0 check-witness python3 -B "$GENERATOR" check-witness \
  "$EXACT_COMP" "$T/exact.omgrsw1"
observe 10 - "$T/exact.omglow3" 0 frame-exact python3 -B "$FRAME" pack \
  "$EXACT_COMP" "$T/exact.omgrsw1"
observe 10 - "$T/adjacent.omglow3" 0 frame-adjacent python3 -B "$FRAME" pack \
  "$ADJACENT_COMP" "$T/exact.omgrsw1"

[ "$(wc -c < "$EXACT_COMP" | tr -d ' ')" -eq 267224 ]
[ "$(wc -c < "$T/exact.omgrsw1" | tr -d ' ')" -eq 461424 ]
[ "$(wc -c < "$T/exact.omglow3" | tr -d ' ')" -eq 728680 ]
[ "$(wc -c < "$T/adjacent.omglow3" | tr -d ' ')" -eq 728681 ]

echo "greatest OMGLOW3: START positive and adjacent observations" >&2
observe 30 "$T/exact.omglow3" "$T/native.ckir3" 0 native-positive "$T/lowerer.native"
observe 30 "$T/exact.omglow3" "$T/self.ckir3" 0 self-positive "$T/lowerer.self"
cmp "$T/native.ckir3" "$T/self.ckir3" >/dev/null
observe 10 - - 0 reference-validate python3 -B "$REFERENCE" validate "$T/native.ckir3"
observe 10 - "$T/result.txt" 0 reference-run python3 -B "$REFERENCE" run "$T/native.ckir3"
[ "$(tr -d ' \r\n' < "$T/result.txt")" = 70 ] || {
  echo "greatest OMGLOW3: independent result is not 70" >&2
  exit 1
}

observe 30 "$T/adjacent.omglow3" "$T/native-adjacent.out" 252 \
  native-adjacent "$T/lowerer.native"
observe 30 "$T/adjacent.omglow3" "$T/self-adjacent.out" 252 \
  self-adjacent "$T/lowerer.self"

python3 -B "$GENERATOR" report "$T/timings.tsv"
echo "greatest OMGLOW3 limits: source-machines=128/128 blocks=2048/2048 "
echo "  raw-types=8192/8192 input-frame=728680/791600 adjacent=728681:252"
echo "greatest OMGLOW3 host procedures: resolver=$RESOLVER_PROCEDURES/128 "
echo "  lowerer=$LOWERER_PROCEDURES/128 lowermachine=$LOWERMACHINE_PROCEDURES/128"
echo "greatest OMGLOW3 adjacent precedence: nested-bundle 263313/263312 first; source aggregate 262145/262144 also exceeded"
echo "greatest OMGLOW3: exact native/self CKIR, independent result 70, and adjacent 252 passed"
