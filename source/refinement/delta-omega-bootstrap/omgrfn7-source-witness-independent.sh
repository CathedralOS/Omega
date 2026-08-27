#!/usr/bin/env sh
# Focused lower-rooted OMGRFN7 responsibility-2 source -> OMGRSW3 gate.
set -eu

STARTED=$(date +%s)
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN7 responsibility 2: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN7 responsibility 2: skipped ($TOOL absent)"; exit 0
  }
done

R="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT"
CORE="$R/omgrfn4-source-witness-independent.beta"
CHECKER="$R/omgrfn7-source-witness-independent.beta"
PACKER="$R/omgrfn7_bundle.py"
BUILDER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
for FILE in "$CORE" "$CHECKER" "$PACKER" "$BUILDER" "$RESOLVER" "$OMEGA_PATH_BETA/bc.beta"; do
  [ -f "$FILE" ] || { echo "OMGRFN7 responsibility 2: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN7_R2_TEMP:-0}" = 1 ]; then
  echo "OMGRFN7 responsibility 2: retained $T" >&2
else
  trap 'rm -rf "$T"' EXIT
fi
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"

awk '1' "$CORE" "$CHECKER" > "$T/core.beta"
printf '%s\n' 'proc main() { return omgrfn5_r2_check() }' > "$T/main.beta"
awk '1' "$T/core.beta" "$T/main.beta" > "$T/check.beta"
python3 -B - "$T/check.beta" "$T/check-pruned.beta" <<'PY'
from pathlib import Path
import re, sys

source = Path(sys.argv[1]).read_text(encoding="ascii")
procedures = {}
order = []
for match in re.finditer(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*\{", source):
    depth = 1
    cursor = match.end()
    while depth:
        depth += (source[cursor] == "{") - (source[cursor] == "}")
        cursor += 1
    name = match.group(1)
    if name in procedures:
        raise SystemExit("duplicate procedure " + name)
    procedures[name] = source[match.start():cursor].rstrip() + "\n"
    order.append(name)
reachable = set()
pending = ["main"]
while pending:
    name = pending.pop()
    if name in reachable:
        continue
    if name not in procedures:
        raise SystemExit("missing reachable procedure " + name)
    reachable.add(name)
    for called in re.findall(r"\b([A-Za-z_]\w*)\s*\(", procedures[name]):
        if called in procedures and called not in reachable:
            pending.append(called)
Path(sys.argv[2]).write_text(
    "\n".join(procedures[name] for name in order if name in reachable),
    encoding="ascii",
)
PY
mv "$T/check-pruned.beta" "$T/check.beta"
PROCEDURES=$(awk '/^proc / { n += 1 } END { print n + 0 }' "$T/check.beta")
[ "$PROCEDURES" -le 128 ] || { echo "OMGRFN7 R2 procedures $PROCEDURES" >&2; exit 1; }
MAX_LOCALS=$(python3 - "$T/check.beta" <<'PY'
import re, sys
s=open(sys.argv[1], encoding="utf-8").read(); maximum=0
for m in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{", s, re.M):
    end=s.find("\nproc ", m.end()); body=s[m.end():end if end >= 0 else len(s)]
    maximum=max(maximum, sum(bool(x.strip()) for x in m.group(1).split(",")) + len(re.findall(r"\blet\s+[A-Za-z_]\w*", body)))
print(maximum)
PY
)
[ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN7 R2 locals $MAX_LOCALS" >&2; exit 1; }

stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"
"$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
BC1_TAPE=$(wc -c < "$T/bc1.tape" | tr -d ' ')
[ $((BC1_TAPE + 4)) -le "$HOLE_SIZE" ] || { echo "OMGRFN7 R2 self compiler exceeds hole" >&2; exit 1; }
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
"$T/bc0" < "$T/check.beta" > "$T/native.asm"
"$T/bc1" < "$T/check.beta" > "$T/self.asm"
cmp "$T/native.asm" "$T/self.asm" >/dev/null
"$ASM" < "$T/native.asm" > "$T/native.tape"
"$ASM" < "$T/self.asm" > "$T/self.tape"
cmp "$T/native.tape" "$T/self.tape" >/dev/null
TAPE_BYTES=$(wc -c < "$T/native.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || { echo "OMGRFN7 R2 tape $TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/native.tape" "$SEED" "$T/native" >/dev/null 2>&1
stamp_seed "$T/self.tape" "$SEED" "$T/self" >/dev/null 2>&1

python3 - "$T/declarations.omg" "$T/root.omg" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_text("""\
data Leaf [copy] { value: u8; }
data Event [copy] {
    case None;
    case Byte(value: u8);
    case Product(a: bool, b: bool, c: bool, leaf: Leaf);
}
data Cell [copy] { event: Event; }
data Probe { prefix: u8; cell: Cell; flag: bool; full: u32; }
machine Cell::read(&self) -> u8 { 70 }
""", encoding="ascii")
Path(sys.argv[2]).write_text(
    "machine Probe::run(&mut self) -> u8 { self.cell.read() }\n",
    encoding="ascii",
)
PY

python3 -B "$BUILDER" build "$T/exact.omgc" Probe run "$T/declarations.omg" "$T/root.omg"
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
"$T/resolver" < "$T/exact.omgc" > "$T/exact.witness"
python3 - "$T/exact.witness" <<'PY'
from pathlib import Path
import struct, sys
r=Path(sys.argv[1]).read_bytes()
if len(r)<84 or r[:8] != b"OMGRSW3\0" or struct.unpack_from("<4H",r,8)!=(3,0,0,84):
    raise SystemExit("OMGRFN7 R2 producer did not select exact OMGRSW3")
PY
python3 - "$T/ckir5" <<'PY'
from pathlib import Path
import struct, sys
Path(sys.argv[1]).write_bytes(struct.pack("<8sHH", b"OMGCKIR\0", 5, 0))
PY
printf 'opaque ELF' > "$T/elf"
python3 "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/ckir5" "$T/elf" --result 70 > "$T/exact.rfn"

observe_one() {
  EXE=$1 EXPECTED=$2 FRAME=$3 LABEL=$4
  set +e
  "$EXE" < "$FRAME" > "$T/out" 2> "$T/err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || { echo "OMGRFN7 R2: $LABEL got $ACTUAL expected $EXPECTED" >&2; sed -n '1,10p' "$T/err" >&2; exit 1; }
  [ ! -s "$T/out" ] || { echo "OMGRFN7 R2: $LABEL published stdout" >&2; exit 1; }
}
observe() { observe_one "$T/native" "$1" "$2" "$3 native"; observe_one "$T/self" "$1" "$2" "$3 self"; }
observe 0 "$T/exact.rfn" "canonical pure-sum source/witness"

python3 - "$T/exact.rfn" "$T/witness-mut.rfn" "$T/ckir-mut.rfn" "$T/elf-mut.rfn" "$T/result-mut.rfn" "$T/as-v6.rfn" <<'PY'
from pathlib import Path
import struct, sys
raw=bytearray(Path(sys.argv[1]).read_bytes())
_,_,_,oc,ow,ck,el,_,_=struct.unpack_from("<8s8I",raw)
w=40+oc; c=w+ow; e=c+ck
x=bytearray(raw); x[w+64+8]=1; Path(sys.argv[2]).write_bytes(x)
x=bytearray(raw); x[c+8]=4; Path(sys.argv[3]).write_bytes(x)
x=bytearray(raw); x[e]^=1; Path(sys.argv[4]).write_bytes(x)
x=bytearray(raw); struct.pack_into("<II",x,32,71,71); Path(sys.argv[5]).write_bytes(x)
x=bytearray(raw); x[6]=ord("6"); struct.pack_into("<I",x,8,6); Path(sys.argv[6]).write_bytes(x)
PY
observe 251 "$T/witness-mut.rfn" "sum-count witness mutation"
observe 0 "$T/ckir-mut.rfn" "CKIR identity opaque"
observe 0 "$T/elf-mut.rfn" "ELF opaque"
observe 0 "$T/result-mut.rfn" "claimed result opaque"
observe 251 "$T/as-v6.rfn" "OMGRFN6 outer cross-pair"

ELAPSED=$(( $(date +%s) - STARTED ))
echo "OMGRFN7 responsibility 2: independent two-unit OMGRSW3 sums/cases/named payloads, nominal prefix, recursive copy/acyclic checks, direct field resolution, exact comparison, cross-pair and opacity controls passed native/self"
echo "OMGRFN7 responsibility 2 resources: ${PROCEDURES}/128 procedures; ${MAX_LOCALS}/32 locals; ${TAPE_BYTES}/262140 tape; ${BC1_TAPE}+4/${HOLE_SIZE} self-Beta; elapsed=${ELAPSED}s"
