#!/usr/bin/env sh
# Focused persisted-Beta OMGRFN3 CKIR2/result -> exact Linux x86-64 ELF gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN3 layer 5 ELF: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN3 layer 5 ELF: skipped ($TOOL absent)"
    exit 0
  }
done

ENVELOPE="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3-component-envelope.beta"
ARTIFACT="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir2-refinement-artifact.beta"
ELF_CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir2-refinement-elf.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3_bundle.py"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v2-to-elf.alp"
REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/ckir2_call_reference.py"
SEMANTICS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_v2_reference.py"
ELF_REFERENCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_elf_v2_reference.py"
for REQUIRED in "$ENVELOPE" "$ARTIFACT" "$ELF_CHECKER" "$PACKER" \
  "$BACKEND" "$REFERENCE" "$SEMANTICS" "$ELF_REFERENCE"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN3 layer 5 ELF: missing $REQUIRED" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(date +%s)
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null
{
  sed -n '1,$p' "$ENVELOPE"
  sed '/^proc main()/,$d' "$ARTIFACT"
  sed '/^proc main()/,$d' "$ELF_CHECKER"
  printf '%s\n' \
    'proc main() {' \
    '    let status=omgrfn3_component_read()' \
    '    state envelope { to done when (status != 0)  status=ckir_refinement_artifact_check()  to artifact }' \
    '    state artifact { to done when (status != 0)  status=ckir2_refinement_elf_check()  to done }' \
    '    state done { return status }' \
    '}'
} > "$T/check.beta"
PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/check.beta")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN3 layer 5 ELF: composed checker exceeds 128 procedures ($PROCEDURES)" >&2
  exit 1
}
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

# Published maxima make every private reconstruction region statically finite,
# mutually disjoint, and below Alpha's 0x04000000-byte memory size.
python3 - <<'PY'
regions = [
    (10_800_000, 10_800_128, "reachable"),
    (10_801_024, 10_802_048, "frame sizes"),
    (10_803_000, 10_804_024, "scratch bases"),
    (10_810_000, 10_826_384, "call graph"),
    (10_830_000, 10_831_024, "indegrees"),
    (10_831_100, 10_831_228, "processed"),
    (10_832_000, 10_833_024, "topological order"),
    (10_834_000, 10_835_024, "live stack"),
    (10_836_000, 10_837_024, "reach queue"),
    (11_000_000, 11_016_384, "block offsets"),
    (11_100_000, 11_394_912, "value slots"),
    (11_400_000, 11_662_144, "place slots"),
]
ordered = sorted(regions)
for left, right in zip(ordered, ordered[1:]):
    assert left[1] <= right[0], (left, right)
assert ordered[-1][1] <= 0x04000000
PY

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend" >/dev/null

printf 'opaque-layer5-omgcomp' > "$T/omgcomp"
printf 'opaque-layer5-witness' > "$T/witness"

observe() { # expected input label
  EXPECTED=$1 INPUT=$2 LABEL=$3
  set +e
  "$T/check" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN3 layer 5 ELF: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN3 layer 5 ELF: $LABEL published stdout" >&2
    exit 1
  }
}

pack() { # ckir elf result output
  python3 "$PACKER" "$T/omgcomp" "$T/witness" "$1" "$2" --result "$3" > "$4"
}

python3 "$REFERENCE" emit "$T/canonical.ckir"
"$T/backend" < "$T/canonical.ckir" > "$T/canonical.elf"
python3 "$ELF_REFERENCE" check "$T/canonical.ckir" "$T/canonical.elf" >/dev/null
pack "$T/canonical.ckir" "$T/canonical.elf" 70 "$T/canonical.rfn"
observe 0 "$T/canonical.rfn" "canonical reachable call closure"

# A distinct valid explicit root emits only the formerly unreachable decoy.
python3 - "$T/canonical.ckir" "$T/decoy.ckir" <<'PY'
from pathlib import Path
import struct, sys
raw = bytearray(Path(sys.argv[1]).read_bytes())
struct.pack_into("<I", raw, 16, 3)
Path(sys.argv[2]).write_bytes(raw)
PY
"$T/backend" < "$T/decoy.ckir" > "$T/decoy.elf"
python3 "$ELF_REFERENCE" check "$T/decoy.ckir" "$T/decoy.elf" >/dev/null
pack "$T/decoy.ckir" "$T/decoy.elf" 7 "$T/decoy.rfn"
pack "$T/canonical.ckir" "$T/decoy.elf" 70 "$T/canonical-decoy.rfn"
pack "$T/decoy.ckir" "$T/canonical.elf" 7 "$T/decoy-canonical.rfn"
observe 0 "$T/decoy.rfn" "explicit decoy reachable closure"
observe 251 "$T/canonical-decoy.rfn" "canonical CKIR/decoy closure cross-pair"
observe 251 "$T/decoy-canonical.rfn" "decoy CKIR/canonical closure cross-pair"

# Pin independent instruction sites for caller staging, rdi/rsi setup, rel32
# calls, callee argument loading, eax result storage, and exact image bytes.
python3 - "$T/canonical.elf" "$T" <<'PY'
from pathlib import Path
import sys

raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
text = raw[4096:]

patterns = {
    "arg-stage": b"\x89\x85",
    "receiver-rdi": b"\x48\x89\xc7",
    "args-rsi": b"\x48\x8d\xb5",
    "callee-arg-load": b"\x8b\x86\x00\x00\x00\x00",
    "result-eax-store": b"\x89\x85",
}
sites = {}
for name, pattern in patterns.items():
    positions = []
    at = 0
    while True:
        at = text.find(pattern, at)
        if at < 0: break
        positions.append(at)
        at += 1
    if not positions:
        raise SystemExit(f"missing ABI pattern {name}")
    sites[name] = positions[-1] if name == "result-eax-store" else positions[0]

calls = []
at = 0
while True:
    setup = text.find(b"\x48\x8d\xb5", at)
    if setup < 0: break
    call = text.find(b"\xe8", setup + 3, setup + 16)
    if call < 0:
        raise SystemExit(f"rsi setup at {setup} lacks adjacent call")
    calls.append(call)
    at = call + 1
if len(calls) != 2:
    raise SystemExit(f"expected two ABI calls, got {calls}")
sites["call-rel32"] = calls[0] + 1
result_store = calls[0] + 5
if text[result_store:result_store+2] != b"\x89\x85":
    raise SystemExit("scalar call result is not staged from eax")
sites["result-eax-store"] = result_store
receiver = sites["receiver-rdi"]
stage = text.rfind(b"\x89\x85", max(0, receiver - 32), receiver)
if stage < 0:
    raise SystemExit("scalar argument staging store absent before rdi setup")
sites["arg-stage"] = stage
sites["elf-header"] = 24 - 4096
for name, relative in sites.items():
    absolute = relative + 4096
    if name == "elf-header": absolute = 24
    changed = bytearray(raw)
    changed[absolute] ^= 1
    out.joinpath(name + ".elf").write_bytes(changed)
out.joinpath("truncated.elf").write_bytes(raw[:-1])
out.joinpath("trailing.elf").write_bytes(raw + b"\0")
PY
for CASE in arg-stage receiver-rdi args-rsi callee-arg-load result-eax-store \
  call-rel32 elf-header truncated trailing; do
  pack "$T/canonical.ckir" "$T/$CASE.elf" 70 "$T/$CASE.rfn"
  observe 251 "$T/$CASE.rfn" "$CASE exact reconstruction"
done

pack "$T/canonical.ckir" "$T/canonical.elf" 71 "$T/wrong-result.rfn"
pack "$T/canonical.ckir" "$T/canonical.elf" 326 "$T/same-exit.rfn"
observe 251 "$T/wrong-result.rfn" "wrong full result"
observe 251 "$T/same-exit.rfn" "same exit projection, wrong full result"

# Construct two individually legal ~128 KiB frames whose acyclic call path
# exceeds the 262,144-byte maximum live stack. This is valid CKIR2 and only the
# backend/refinement resource relation rejects it as 252.
python3 - "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" "$T/live.ckir" <<'PY'
from pathlib import Path
import sys
sys.path.insert(0, sys.argv[1])
import ckir2_call_reference as f

NO = 0xffffffff
n = 16380
types = [(0,4,0,0,0,0,0,0),(1,3,0,0,0,0,0,1),(2,2,0,0,0,0,0,0x7fffffff),(3,1,0,0,0,0,0,255)]
records = [(0,0,0,0,0)]
machines = [
    (0,0,2,0,0,3,0,0,0,1,0),
    (1,0,2,0,0,3,0,1,1,1,1),
]
mparams = [(0,1,0,3,0)]
blocks = [
    (0,0,2,0,0,0,0,0,n+2,0),
    (1,1,2,0,0,0,0,n+2,n,1),
]
ops = []
for i in range(n): ops.append((i,0,0,2,2,0,i,0,0,0,0,0))
ops.append((n,0,0,1,1,0,1,3,0,0,68,0))
ops.append((n+1,0,0,10,1,0,2,3,0,2,1,0))
for i in range(n): ops.append((n+2+i,1,1,2,2,0,n+i,0,2,0,0,0))
operands = [(0,),(1,)]
terms = [
    (0,0,0,4,0,0,2,NO,2,0,NO,2,0),
    (1,1,1,4,0,0,0,NO,2,0,NO,2,0),
]
tables=(types,records,[],machines,mparams,blocks,[],ops,operands,terms)
payload=b"".join(row.pack(*item) for table,row in zip(tables,f.ROWS) for item in table)
counts=tuple(len(table) for table in tables)
Path(sys.argv[2]).write_bytes(f.HEADER.pack(b"OMGCKIR\0",2,0,1,1,0,f.HEADER.size+len(payload),*counts,3,2*n)+payload)
PY
python3 "$SEMANTICS" validate "$T/live.ckir" >/dev/null
printf 'nonempty-resource-elf' > "$T/live.elf"
pack "$T/live.ckir" "$T/live.elf" 68 "$T/live.rfn"
observe 252 "$T/live.rfn" "maximum live-stack exhaustion"

ELAPSED=$(($(date +%s) - STARTED))
echo "OMGRFN3 layer 5 ELF: $PROCEDURES procedures; reachable closure, per-machine frames, rdi/rsi/eax ABI, rel32, live-stack, exact ELF/result, and 0/251/252 controls passed below Delta (${ELAPSED}s)"
