#!/usr/bin/env sh
# Focused lower-rooted OMGRFN2 layer-3 witness -> CKIR table/layout gate.
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
  *) echo "OMGRFN2 layer 3: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN2 layer 3: skipped ($TOOL absent)"
    exit 0
  }
done

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2-witness-ckir-tables.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2_bundle.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir.alp"
LOW_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow.py"
MUTATIONS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/resolved_to_ckir_mutations.py"
for REQUIRED in "$CHECKER" "$PACKER" "$FIXTURE" "$RESOLVER" "$LOWERER" \
  "$LOW_FRAME" "$MUTATIONS"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN2 layer 3: missing $REQUIRED" >&2; exit 1; }
done

PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$CHECKER")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN2 layer 3: persisted Beta checker exceeds 128 procedures ($PROCEDURES)" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(date +%s)
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null
"$BC" < "$CHECKER" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null

python3 "$FIXTURE" build "$T/canonical"
python3 "$MUTATIONS" parameter-envelope "$T/parameter.omgc"
python3 - "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" "$T/layout.omgc" "$T/type-order.omgc" <<'PY'
from pathlib import Path
import sys

sys.path.insert(0, sys.argv[1])
from resolution_handoff_reference import one_source

source = """module app;
data Exact {
    left: [u8; 65536];
    right: [u8; 65536];
}
machine Exact::run(&self) -> u8 { 0 }
"""
Path(sys.argv[2]).write_bytes(one_source(source, module="app", owner="Exact"))
type_order = """module app;
data Types {
    first: u8;
    second: u32 [0..=7];
}
machine Types::run(&self) -> u8 { 0 }
"""
Path(sys.argv[3]).write_bytes(one_source(type_order, module="app", owner="Types"))
PY

run_expect() (
  EXE=$1
  INPUT=$2
  EXPECTED=$3
  OUTPUT=$4
  LABEL=$5
  set +e
  "$EXE" < "$INPUT" > "$OUTPUT" 2> "$OUTPUT.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN2 layer 3: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,20p' "$OUTPUT.stderr" >&2
    exit 1
  }
  if [ "$EXPECTED" -ne 0 ] && [ -s "$OUTPUT" ]; then
    echo "OMGRFN2 layer 3: $LABEL published bytes on rejection" >&2
    exit 1
  fi
)

build_pair() (
  LABEL=$1
  OMGCOMP=$2
  RESULT=$3
  run_expect "$T/resolver" "$OMGCOMP" 0 "$T/$LABEL.witness" "$LABEL resolver"
  python3 "$LOW_FRAME" pack "$OMGCOMP" "$T/$LABEL.witness" > "$T/$LABEL.omglow"
  run_expect "$T/lowerer" "$T/$LABEL.omglow" 0 "$T/$LABEL.ckir" "$LABEL lowerer"
  printf 'opaque-layer3-elf' > "$T/$LABEL.elf"
  python3 "$PACKER" "$OMGCOMP" "$T/$LABEL.witness" "$T/$LABEL.ckir" \
    "$T/$LABEL.elf" --result "$RESULT" > "$T/$LABEL.rfn"
  run_expect "$T/check" "$T/$LABEL.rfn" 0 "$T/$LABEL.out" "$LABEL table/layout join"
)

build_pair canonical "$T/canonical/compilation-envelope.bin" 70
build_pair parameter "$T/parameter.omgc" 0
build_pair layout "$T/layout.omgc" 0
build_pair type-order "$T/type-order.omgc" 0

# Two independently valid witness/CKIR products must not cross-pair.
python3 "$PACKER" "$T/canonical/compilation-envelope.bin" "$T/canonical.witness" \
  "$T/parameter.ckir" "$T/canonical.elf" --result 70 > "$T/canonical-witness-parameter-ckir.rfn"
python3 "$PACKER" "$T/parameter.omgc" "$T/parameter.witness" \
  "$T/canonical.ckir" "$T/parameter.elf" --result 0 > "$T/parameter-witness-canonical-ckir.rfn"
run_expect "$T/check" "$T/canonical-witness-parameter-ckir.rfn" 251 \
  "$T/cross-a.out" "valid canonical-witness/parameter-CKIR cross-pair"
run_expect "$T/check" "$T/parameter-witness-canonical-ckir.rfn" 251 \
  "$T/cross-b.out" "valid parameter-witness/canonical-CKIR cross-pair"

python3 - "$T/canonical.rfn" "$T/parameter.rfn" "$T/layout.rfn" \
  "$T/type-order.rfn" "$T/cases" <<'PY'
from pathlib import Path
import struct
import sys

canonical = Path(sys.argv[1]).read_bytes()
parameter = Path(sys.argv[2]).read_bytes()
layout = Path(sys.argv[3]).read_bytes()
type_order = Path(sys.argv[4]).read_bytes()
out = Path(sys.argv[5])
out.mkdir()
U32 = struct.Struct("<I")

def split(raw):
    magic, version, flags, cn, wn, kn, en, result, exit_code = struct.unpack_from("<8s8I", raw)
    assert magic == b"OMGRFN2\0" and version == 2
    at = 40
    comp = raw[at:at+cn]; at += cn
    witness = raw[at:at+wn]; at += wn
    ckir = raw[at:at+kn]; at += kn
    elf = raw[at:at+en]; at += en
    assert at == len(raw)
    return flags, result, comp, witness, ckir, elf

def pack(parts, *, witness=None, ckir=None, elf=None, result=None):
    flags, old_result, comp, old_witness, old_ckir, old_elf = parts
    witness = old_witness if witness is None else witness
    ckir = old_ckir if ckir is None else ckir
    elf = old_elf if elf is None else elf
    result = old_result if result is None else result
    return struct.pack("<8s8I", b"OMGRFN2\0", 2, flags, len(comp), len(witness),
                       len(ckir), len(elf), result, result & 255) + comp + witness + ckir + elf

def wmeta(w):
    counts = struct.unpack_from("<11I", w, 20)
    names = ("units", "imports", "bindings", "declarations", "types", "records",
             "fields", "machines", "mparams", "blocks", "bparams")
    strides = (36, 48, 28, 28, 24, 24, 24, 40, 24, 40, 24)
    offsets = {}
    at = 72
    for name, count, stride in zip(names, counts, strides):
        offsets[name] = at
        at += count * stride
    assert at == len(w)
    return dict(zip(names, counts)), offsets

def cmeta(c):
    counts = struct.unpack_from("<12I", c, 24)
    names = ("types", "records", "fields", "machines", "mparams", "blocks",
             "bparams", "operations", "operands", "terminators", "values", "places")
    strides = (24, 20, 16, 36, 20, 32, 20, 40, 4, 44)
    offsets = {}
    at = 72
    for name, count, stride in zip(names[:10], counts[:10], strides):
        offsets[name] = at
        at += count * stride
    assert at == len(c)
    return dict(zip(names, counts)), offsets

def changed(raw, at, value):
    result = bytearray(raw); result[at] = value; return bytes(result)

def word(raw, at, value):
    result = bytearray(raw); U32.pack_into(result, at, value); return bytes(result)

def put(name, raw): (out / name).write_bytes(raw)

cp = split(canonical); pp = split(parameter); lp = split(layout); tp = split(type_order)
cc, co = cmeta(cp[4]); wc, wo = wmeta(cp[3])
pc, pco = cmeta(pp[4]); pwc, pwo = wmeta(pp[3])
lc, lco = cmeta(lp[4]); lwc, lwo = wmeta(lp[3])
tc, tco = cmeta(tp[4]); twc, two = wmeta(tp[3])

# The layer boundary is observable: operation spans/rows, claims, and ELF are
# deliberately opaque here and remain obligations of layers 4 and 5.
assert cc["operations"] > 0
opaque_block = word(cp[4], co["blocks"] + 20, 0x7fffffff)
put("opaque-block-operation-span", pack(cp, ckir=opaque_block))
opaque_operation = changed(cp[4], co["operations"] + 8, 255)
put("opaque-operation-row", pack(cp, ckir=opaque_operation))
put("opaque-claimed-result", pack(cp, result=71))
put("opaque-elf", pack(cp, elf=changed(cp[5], 0, cp[5][0] ^ 1)))

# Independent relation teeth for every owned row family.
put("bad-type-row", pack(cp, witness=word(cp[3], wo["types"] + wc["records"]*24 + 20, 2)))
put("bad-record-join", pack(cp, witness=changed(cp[3], wo["records"] + 20, 0)))
put("bad-field-owner", pack(cp, witness=word(cp[3], wo["fields"] + 4, 1)))
put("bad-machine-owner", pack(cp, witness=word(cp[3], wo["machines"] + 8, wc["records"])))
put("bad-block-owner", pack(cp, witness=word(cp[3], wo["blocks"] + 4, wc["machines"])))
put("bad-selected-root", pack(cp, witness=word(cp[3], 64, 0xffffffff)))
put("bad-ckir-type-join", pack(cp, ckir=changed(cp[4], co["types"] + 4, 1)))

# Keep both carriers internally well-shaped while swapping two distinct
# non-array descriptors and every layer-3 reference.  Only reconstruction of
# first authored encounter order can reject this relation.
base = twc["records"] + 2
assert twc["types"] == base + 2
w = bytearray(tp[3]); c = bytearray(tp[4])
wrow0 = bytes(w[two["types"] + base*24 + 4:two["types"] + base*24 + 24])
wrow1 = bytes(w[two["types"] + (base+1)*24 + 4:two["types"] + (base+1)*24 + 24])
crow0 = bytes(c[tco["types"] + base*24 + 4:tco["types"] + base*24 + 24])
crow1 = bytes(c[tco["types"] + (base+1)*24 + 4:tco["types"] + (base+1)*24 + 24])
w[two["types"] + base*24 + 4:two["types"] + base*24 + 24] = wrow1
w[two["types"] + (base+1)*24 + 4:two["types"] + (base+1)*24 + 24] = wrow0
c[tco["types"] + base*24 + 4:tco["types"] + base*24 + 24] = crow1
c[tco["types"] + (base+1)*24 + 4:tco["types"] + (base+1)*24 + 24] = crow0
def remap_rows(raw, offset, count, stride, field):
    for row in range(count):
        at = offset + row*stride + field
        value = U32.unpack_from(raw, at)[0]
        if value == base: U32.pack_into(raw, at, base+1)
        elif value == base+1: U32.pack_into(raw, at, base)
remap_rows(w, two["fields"], twc["fields"], 24, 12)
remap_rows(w, two["mparams"], twc["mparams"], 24, 12)
remap_rows(w, two["machines"], twc["machines"], 40, 16)
remap_rows(w, two["bparams"], twc["bparams"], 24, 12)
remap_rows(c, tco["fields"], tc["fields"], 16, 12)
remap_rows(c, tco["mparams"], tc["mparams"], 20, 12)
remap_rows(c, tco["machines"], tc["machines"], 36, 12)
remap_rows(c, tco["bparams"], tc["bparams"], 20, 12)
put("noncanonical-type-interning-order", pack(tp, witness=bytes(w), ckir=bytes(c)))

assert pwc["mparams"] > 0 and pwc["bparams"] > 0
put("bad-machine-parameter-owner", pack(pp, witness=word(pp[3], pwo["mparams"] + 4, pwc["machines"])))
put("bad-block-parameter-owner", pack(pp, witness=word(pp[3], pwo["bparams"] + 4, 0)))
w = word(pp[3], pwo["mparams"] + 12, 0)
c = word(pp[4], pco["mparams"] + 12, 0)
put("noncopyable-structural-parameter", pack(pp, witness=w, ckir=c))

# Copyability and by-value acyclicity are independently reconstructed even
# though neither relation is encoded in CKIR as an asserted layout.
w = bytearray(cp[3]); c = bytearray(cp[4])
U32.pack_into(w, wo["fields"] + 2*24 + 12, 1)  # Probe.pair becomes Probe.
U32.pack_into(c, co["fields"] + 2*16 + 12, 1)
put("recursive-by-value-layout", pack(cp, witness=bytes(w), ckir=bytes(c)))
w = bytearray(cp[3]); c = bytearray(cp[4])
w[wo["records"] + 20] = 0; c[co["records"] + 16] = 0
w[wo["records"] + 24 + 20] = 1; c[co["records"] + 20 + 16] = 1
put("noncopyable-marked-record", pack(cp, witness=bytes(w), ckir=bytes(c)))

# Resource precedence: malformed extents after an over-limit validated count
# cannot downgrade 252.  The exact 131,072-byte layout above is the positive
# adjacent control; moving either exact array length one step over is 252.
put("witness-type-count-2049", pack(cp, witness=word(cp[3], 36, 2049)))
put("ckir-type-count-8193", pack(cp, ckir=word(cp[4], 24, 8193)))
put("malformed-witness-count-no-id", pack(cp, witness=word(cp[3], 36, 0xffffffff)))
put("malformed-ckir-count-no-id", pack(cp, ckir=word(cp[4], 24, 0xffffffff)))
put("declared-omgcomp-far-over", word(canonical, 16, 16777216))
array_ids = [i for i in range(lwc["types"]) if lp[3][lwo["types"] + i*24 + 4] == 5]
assert array_ids
array_id = array_ids[0]
w = word(lp[3], lwo["types"] + array_id*24 + 12, 65537)
c = word(lp[4], lco["types"] + array_id*24 + 12, 65537)
put("array-length-65537", pack(lp, witness=w, ckir=c))
PY

for CASE in opaque-block-operation-span opaque-operation-row opaque-claimed-result opaque-elf; do
  run_expect "$T/check" "$T/cases/$CASE" 0 "$T/$CASE.out" "$CASE boundary control"
done
for CASE in bad-type-row bad-record-join bad-field-owner bad-machine-owner \
  bad-block-owner bad-selected-root bad-ckir-type-join \
  noncanonical-type-interning-order \
  bad-machine-parameter-owner bad-block-parameter-owner \
  noncopyable-structural-parameter recursive-by-value-layout \
  noncopyable-marked-record malformed-witness-count-no-id \
  malformed-ckir-count-no-id; do
  run_expect "$T/check" "$T/cases/$CASE" 251 "$T/$CASE.out" "$CASE relation control"
done
for CASE in witness-type-count-2049 ckir-type-count-8193 array-length-65537 \
  declared-omgcomp-far-over; do
  run_expect "$T/check" "$T/cases/$CASE" 252 "$T/$CASE.out" "$CASE resource control"
done

ELAPSED=$(($(date +%s) - STARTED))
echo "OMGRFN2 layer 3: witness/CKIR tables, canonical interning, copy/layout, root, cross-pairs, opaque later layers, and 0/251/252 teeth passed below Delta (${ELAPSED}s)"
