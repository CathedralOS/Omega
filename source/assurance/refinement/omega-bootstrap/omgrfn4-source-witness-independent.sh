#!/usr/bin/env sh
# Focused OMGRFN4 responsibility-2 source -> OMGRSW1 refinement gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN4 responsibility 2: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN4 responsibility 2: skipped ($TOOL absent)"
    exit 0
  }
done

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4-source-witness-independent.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4_bundle.py"
OLD_PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3_bundle.py"
BUILDER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
UNICODE="$OMEGA_REPO_ROOT/source/compiler/omega/psi/generated/unicode_tables.omg"
HARNESS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir3-constant-aggregates/unicode-harness.omg"
for REQUIRED in "$CHECKER" "$PACKER" "$OLD_PACKER" "$BUILDER" "$RESOLVER" "$UNICODE" "$HARNESS"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN4 responsibility 2: missing $REQUIRED" >&2; exit 1; }
done

PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$CHECKER")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN4 responsibility 2: exceeds 128 procedures ($PROCEDURES)" >&2
  exit 1
}
MAX_LOCALS=$(python3 - "$CHECKER" <<'PY'
import re, sys
source = open(sys.argv[1], encoding="utf-8").read()
maximum = 0
for match in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{", source, re.M):
    end = source.find("\nproc ", match.end())
    body = source[match.end():end if end >= 0 else len(source)]
    params = sum(bool(item.strip()) for item in match.group(1).split(","))
    maximum = max(maximum, params + len(re.findall(r"\blet\s+[A-Za-z_]\w*", body)))
print(maximum)
PY
)
[ "$MAX_LOCALS" -le 32 ] || {
  echo "OMGRFN4 responsibility 2: exceeds 32 local metadata slots ($MAX_LOCALS)" >&2
  exit 1
}
grep -q 'count>=18000' "$CHECKER" || {
  echo "OMGRFN4 responsibility 2: token evidence ceiling drifted" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(date +%s)
echo "OMGRFN4 responsibility 2: compiling bounded persisted-Beta checker"
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null
cp "$CHECKER" "$T/check.beta"
printf '\nproc main() { return omgrfn4_r2_check() }\n' >> "$T/check.beta"
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
TAPE_BYTES=$(wc -c < "$T/check.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || {
  echo "OMGRFN4 responsibility 2: checker tape exceeds 262140 bytes ($TAPE_BYTES)" >&2
  exit 1
}
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

echo "OMGRFN4 responsibility 2: building exact Unicode+harness carrier and canonical OMGRSW1"
cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
python3 -B "$BUILDER" build "$T/canonical.omgc" UnicodeTables \
  bootstrap_constant_aggregate_probe "$UNICODE" "$HARNESS"
"$T/resolver" < "$T/canonical.omgc" > "$T/canonical.witness"
[ "$(wc -c < "$T/canonical.omgc" | tr -d ' ')" -eq 84140 ]
[ "$(wc -c < "$T/canonical.witness" | tr -d ' ')" -eq 3004 ]
printf opaque-ckir3 > "$T/ckir"
printf opaque-elf > "$T/elf"

run_expect() {
  EXE=$1 INPUT=$2 EXPECTED=$3 LABEL=$4
  set +e
  "$EXE" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN4 responsibility 2: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN4 responsibility 2: $LABEL published stdout" >&2
    exit 1
  }
}

pack() {
  PACK_NAME=$1 PACK_COMP=$2 PACK_WITNESS=$3
  python3 "$PACKER" "$PACK_COMP" "$PACK_WITNESS" "$T/ckir" "$T/elf" --result 70 > "$T/$PACK_NAME.rfn"
}

pack canonical "$T/canonical.omgc" "$T/canonical.witness"
run_expect "$T/check" "$T/canonical.rfn" 0 "canonical Unicode+harness resolution"

# Fully renamed declarations and all matching type/call references prove that
# row names and spans are source-derived, not embedded fixture identifiers.
python3 - "$UNICODE" "$HARNESS" "$T/renamed-unicode.omg" "$T/renamed-harness.omg" <<'PY'
from pathlib import Path
import sys
for source, target in ((sys.argv[1], sys.argv[3]), (sys.argv[2], sys.argv[4])):
    raw = Path(source).read_bytes()
    for old, new in (
        (b"UnicodeRange", b"ScalarBounds"),
        (b"UnicodeTables", b"ScalarCatalog"),
        (b"initialize", b"prime_table"),
        (b"is_xid_start", b"lookup_start"),
        (b"is_xid_continue", b"lookup_continue"),
        (b"bootstrap_constant_aggregate_probe", b"renamed_constant_aggregate_probe"),
    ):
        raw = raw.replace(old, new)
    Path(target).write_bytes(raw)
PY
python3 -B "$BUILDER" build "$T/renamed.omgc" ScalarCatalog \
  renamed_constant_aggregate_probe "$T/renamed-unicode.omg" "$T/renamed-harness.omg"
"$T/resolver" < "$T/renamed.omgc" > "$T/renamed.witness"
pack renamed "$T/renamed.omgc" "$T/renamed.witness"
run_expect "$T/check" "$T/renamed.rfn" 0 "fully renamed source-derived rows"

echo "OMGRFN4 responsibility 2: exercising exact row/order/span/binding teeth"
python3 - "$T/canonical.witness" "$T/mutations" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2]); out.mkdir()
bindings = 72 + 2 * 36

def put(name, b): out.joinpath(name + ".witness").write_bytes(b)
def mutate(name, offset, value, fmt="<I"):
    b = bytearray(raw); struct.pack_into(fmt, b, offset, value); put(name, b)

# First role-3 row is binding 6 in canonical source/start order.
role3 = bindings + 6 * 28
mutate("target", role3 + 20, 3)
mutate("span", role3 + 12, struct.unpack_from("<I", raw, role3 + 12)[0] + 1)
mutate("missing-consumption", role3 + 8, 2, "<B")

b = bytearray(raw)
left, right = bytes(b[role3:role3+28]), bytes(b[role3+28:role3+56])
b[role3:role3+28], b[role3+28:role3+56] = right, left
put("order", b)

b = bytearray(raw)
b[role3+28+4:role3+56] = raw[role3+4:role3+28]
put("duplicate-binding", b)

# Unchanged inherited table rows are part of the same exact reconstruction.
types = bindings + 15*28 + 6*28
mutate("type-row", types + 4*24 + 5, 0, "<B")
blocks = types + 8*24 + 2*24 + 5*24 + 4*40 + 2*24
mutate("block-span", blocks + 13*40 + 20,
       struct.unpack_from("<I", raw, blocks + 13*40 + 20)[0] + 1)
PY
for NAME in target span missing-consumption order duplicate-binding type-row block-span; do
  pack "mutation-$NAME" "$T/canonical.omgc" "$T/mutations/$NAME.witness"
  run_expect "$T/check" "$T/mutation-$NAME.rfn" 251 "$NAME mutation"
done

# A missing same-owner call target is rejected from source independently; the
# producer agrees and publishes no witness.
python3 - "$HARNESS" "$T/missing-harness.omg" <<'PY'
from pathlib import Path
import sys
raw = Path(sys.argv[1]).read_bytes().replace(b"self.initialize();", b"self.missing_call();", 1)
Path(sys.argv[2]).write_bytes(raw)
PY
python3 -B "$BUILDER" build "$T/missing.omgc" UnicodeTables \
  bootstrap_constant_aggregate_probe "$UNICODE" "$T/missing-harness.omg"
pack missing "$T/missing.omgc" "$T/canonical.witness"
run_expect "$T/check" "$T/missing.rfn" 251 "missing source call binding"
run_expect "$T/resolver" "$T/missing.omgc" 251 "resolver missing source call binding"

# Later components and claimed result bytes are physically present but opaque
# to responsibility 2.
printf changed-ckir3 > "$T/opaque.ckir"
printf changed-elf > "$T/opaque.elf"
python3 "$PACKER" "$T/canonical.omgc" "$T/canonical.witness" \
  "$T/opaque.ckir" "$T/opaque.elf" --result 71 > "$T/opaque.rfn"
run_expect "$T/check" "$T/opaque.rfn" 0 "opaque CKIR3/ELF/result"

# Version identity and phase-local resource behavior.
python3 "$OLD_PACKER" "$T/canonical.omgc" "$T/canonical.witness" \
  "$T/ckir" "$T/elf" --result 70 > "$T/old.rfn"
run_expect "$T/check" "$T/old.rfn" 251 "OMGRFN3 cross-version frame"
python3 - "$T/canonical.rfn" "$T/bad-version.rfn" "$T/witness-over.rfn" <<'PY'
from pathlib import Path
import struct, sys
raw = Path(sys.argv[1]).read_bytes()
b = bytearray(raw); struct.pack_into("<I", b, 8, 3); Path(sys.argv[2]).write_bytes(b)
b = bytearray(raw); struct.pack_into("<I", b, 20, 524289); Path(sys.argv[3]).write_bytes(b)
PY
run_expect "$T/check" "$T/bad-version.rfn" 251 "malformed V4 version"
run_expect "$T/check" "$T/witness-over.rfn" 252 "declared witness exhaustion"

# The persisted checker owns an explicit 18,000-token/source evidence cap.
# This structurally valid source extent selects 252 before unsupported syntax.
python3 - "$T/token-over.omg" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_text(";" * 18001, encoding="ascii")
PY
python3 -B "$BUILDER" build "$T/token-over.omgc" UnicodeTables \
  bootstrap_constant_aggregate_probe "$T/token-over.omg" "$HARNESS"
pack token-over "$T/token-over.omgc" "$T/canonical.witness"
run_expect "$T/check" "$T/token-over.rfn" 252 "source token evidence exhaustion"

ELAPSED=$(($(date +%s) - STARTED))
echo "OMGRFN4 responsibility 2: exact Unicode+harness OMGRSW1, 2 units/15 bindings (9 role-3)/6 declarations/8 types/2 records/5 fields/4 machines/39 blocks, renamed and mutation/resource controls passed below Delta (${ELAPSED}s; ${PROCEDURES}/128 procedures; ${MAX_LOCALS}/32 locals; ${TAPE_BYTES}/262140 tape bytes; 18000 tokens/source)"
