#!/usr/bin/env sh
# Rust-free persisted-Beta/Gamma meaning join for OMGRSWC12-only facilities.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || { echo "token-stream meaning: repository root not found" >&2; exit 2; }
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in "Darwin arm64") ;; *) echo "token-stream meaning: skipped (requires Darwin arm64)"; exit 0 ;; esac
for TOOL in cargo python3; do command -v "$TOOL" >/dev/null 2>&1 || { echo "token-stream meaning: skipped ($TOOL absent)"; exit 0; }; done

INHERITED=$GATE_DIR/delta-record-array-to-ckir19-meaning.sh
RESOLVER=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-token-stream-resolve.alp
PROBE=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-token-stream-meaning-probe.alp
SOURCE_FIXTURE=$GATE_DIR/omgrsw12_token_stream_fixture.py
RUNNER=$GATE_DIR/delta-ckir4-meaning-runner.py
DECODER=$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/decode-gamma-output.py
for REQUIRED in "$INHERITED" "$RESOLVER" "$PROBE" "$SOURCE_FIXTURE" "$RUNNER" "$DECODER"; do
  [ -f "$REQUIRED" ] || { echo "token-stream meaning: missing $REQUIRED" >&2; exit 1; }
done

# OMGRSW11 already gives persisted-Beta/Gamma meaning to the fixed-array,
# indexed scalar-store, Exact increment, full/retain and tag-readback lane.
# It remains an independently runnable required gate; do not replay its slow
# canonical interpreter cases inside every V12 facility-join invocation.

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
stamp_beta_compiler "$T/bc.exe" >/dev/null || { echo "token-stream meaning FAIL - Beta compiler artifact" >&2; exit 1; }
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
build_beta() {
  "$T/bc.exe" < "$1" > "$T/program.asm" 2>/dev/null \
    && "$ASM" < "$T/program.asm" > "$T/program.tape" 2>/dev/null \
    && stamp_seed "$T/program.tape" "$SEED" "$2" >/dev/null 2>&1
}
build_beta "$OMEGA_PATH_OMEGA_BOOTSTRAP/meaning/omega2gamma.beta" "$T/elaborate.exe" || { echo "token-stream meaning FAIL - omega2gamma build" >&2; exit 1; }
build_beta "$OMEGA_PATH_GAMMA/interp.beta" "$T/interp.exe" || { echo "token-stream meaning FAIL - Gamma interpreter build" >&2; exit 1; }
python3 -B "$RUNNER" elaborate "$T/elaborate.exe" "$PROBE" "$T/probe.gamma" "$T/timings.tsv" "token-stream V12 facility join" 60 120000

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$PROBE" "$T/probe.native" >/dev/null
python3 -B "$SOURCE_FIXTURE" build "$T/source"
"$T/resolver.native" < "$T/source/canonical.omgc" > "$T/canonical.omgrswc"
python3 -B "$SOURCE_FIXTURE" inspect "$T/source/canonical.omgc" "$T/canonical.omgrswc"

# Materialize a compact certificate from the accepted witness. Every byte is
# selected from a V12 row/header fact; no hand-authored positive can bypass the
# real resolver publication.
python3 -B - "$T/canonical.omgrswc" "$T" <<'PY'
from pathlib import Path
import struct, sys
w = Path(sys.argv[1]).read_bytes(); out = Path(sys.argv[2])
u32 = lambda at: struct.unpack_from("<I", w, at)[0]
record_copies = sum(u32(916 + row * 32 + 28) for row in range(8))
sum_copies = sum(u32(1868 + row * 32 + 28) for row in range(5))
cases = [u32(1868 + row * 32 + 16) for row in range(5)]
push_params = u32(5160 + 20)
source_value = u32(7168 + 20)
float_case = u32(7168 + 32 + 20)
float_bits = u32(7168 + 32 + 24)
tag = u32(7168 + 6 * 32 + 20)
cert = bytes([record_copies, sum_copies, *cases, push_params,
              u32(72), u32(76), source_value, float_case,
              float_bits & 1, (float_bits >> 1) & 1, (float_bits >> 2) & 1,
              tag, u32(152) & 255, (u32(152) >> 8) & 255,
              0, 1, 2, 1, tag, tag])
assert len(cert) == 24
(out / "canonical.input").write_bytes(cert)
bad = bytearray(cert); bad[11] = 77
(out / "semantic.input").write_bytes(bad)
(out / "resource.input").write_bytes(bytes(65))
PY

native_case() {
  LABEL=$1 EXPECTED=$2
  set +e; "$T/probe.native" < "$T/$LABEL.input" > "$T/$LABEL.expected"; ACTUAL=$?; set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || { echo "token-stream meaning FAIL - $LABEL native $ACTUAL expected $EXPECTED" >&2; exit 1; }
  [ "$EXPECTED" -eq 0 ] || [ ! -s "$T/$LABEL.expected" ] || { echo "token-stream meaning FAIL - $LABEL published on rejection" >&2; exit 1; }
}
native_case canonical 0
native_case semantic 251
native_case resource 252
[ "$(od -An -tu1 "$T/canonical.expected" | tr -d ' ')" = 70 ] || { echo "token-stream meaning FAIL - native join did not return tag70" >&2; exit 1; }

for LABEL in canonical semantic resource; do
  python3 -B "$RUNNER" run "$T/interp.exe" "$T/probe.gamma" "$T/$LABEL.input" "$T/$LABEL.observation" "$T/timings.tsv" "token-stream V12 $LABEL" 240
done
check_gamma() {
  LABEL=$1 EXPECTED=$2
  ACTUAL=$(python3 -B "$DECODER" "$T/$LABEL.observation" "$T/$LABEL.stdout")
  [ "$ACTUAL" -eq "$EXPECTED" ] || { echo "token-stream meaning FAIL - $LABEL Gamma $ACTUAL expected $EXPECTED" >&2; exit 1; }
  cmp "$T/$LABEL.stdout" "$T/$LABEL.expected" >/dev/null || { echo "token-stream meaning FAIL - $LABEL Gamma publication differs" >&2; exit 1; }
}
check_gamma canonical 0
check_gamma semantic 251
check_gamma resource 252

python3 - "$T/timings.tsv" <<'PY'
from pathlib import Path
import sys
rows=[]
for line in Path(sys.argv[1]).read_text(encoding="ascii").splitlines():
    seconds,size,label=line.split("\t",2); rows.append(f"{label}={float(seconds):.2f}s/{size}B")
print("token-stream meaning: inherited OMGRSW11 array/store meaning plus OMGRSWC12 "
      "record/sum copy, 10-arg SourceId+TokenKind, source.value, Float(true,false,true), "
      "dispatch/tag70 and 251/252 no-publication join PASS through persisted-Beta Gamma; " + " ".join(rows))
PY
