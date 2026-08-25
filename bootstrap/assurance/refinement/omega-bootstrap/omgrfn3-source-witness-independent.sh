#!/usr/bin/env sh
# Focused OMGRFN3 layer-2 source -> OMGRSW1 attached-call refinement gate.
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
  *) echo "OMGRFN3 layer 2: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN3 layer 2: skipped ($TOOL absent)"
    exit 0
  }
done

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3-source-witness-independent.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3_bundle.py"
OLD_PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2_bundle.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/role3_resolution_fixture.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
for REQUIRED in "$CHECKER" "$PACKER" "$OLD_PACKER" "$FIXTURE" "$RESOLVER"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN3 layer 2: missing $REQUIRED" >&2; exit 1; }
done

PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$CHECKER")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN3 layer 2: persisted-Beta checker exceeds 128 procedures ($PROCEDURES)" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(date +%s)
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null
cp "$CHECKER" "$T/check.beta"
printf '\nproc main() { return omgrfn3_l2_check() }\n' >> "$T/check.beta"
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null

python3 "$FIXTURE" build "$T/fixtures"
printf x > "$T/ckir"
printf x > "$T/elf"

run_expect() {
  EXE=$1
  INPUT=$2
  EXPECTED=$3
  LABEL=$4
  set +e
  "$EXE" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN3 layer 2: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN3 layer 2: $LABEL published stdout" >&2
    exit 1
  }
}

pack() {
  PACK_NAME=$1
  PACK_OMGCOMP=$2
  PACK_WITNESS=$3
  python3 "$PACKER" "$PACK_OMGCOMP" "$PACK_WITNESS" "$T/ckir" "$T/elf" \
    --result 70 > "$T/$PACK_NAME.rfn"
}

"$T/resolver" < "$T/fixtures/valid.omgc" > "$T/canonical.witness"
python3 "$FIXTURE" check "$T/fixtures/valid.omgc" "$T/canonical.witness"
pack canonical "$T/fixtures/valid.omgc" "$T/canonical.witness"
run_expect "$T/check" "$T/canonical.rfn" 0 "canonical exact role-3 source/witness"

# Same grammar and relations under fully different fixture identifiers and
# module spelling.  Equal-width replacements preserve the envelope layout but
# change both source and manifest strings; the resolver independently rebuilds
# every affected span and row.
python3 - "$T/fixtures/valid.omgc" "$T/renamed.omgc" <<'PY'
from pathlib import Path
import sys
raw = Path(sys.argv[1]).read_bytes()
for old, new in (
    (b"Probe", b"Vault"),
    (b"local", b"inner"),
    (b"cross", b"other"),
    (b"decoy", b"spare"),
    (b"run", b"top"),
    (b"app", b"xyz"),
):
    raw = raw.replace(old, new)
Path(sys.argv[2]).write_bytes(raw)
PY
"$T/resolver" < "$T/renamed.omgc" > "$T/renamed.witness"
pack renamed "$T/renamed.omgc" "$T/renamed.witness"
run_expect "$T/check" "$T/renamed.rfn" 0 "renamed/module-changed independent reconstruction"

# The checker reconstructs all 940 witness bytes.  These focused mutations
# isolate the role-3 target, token span, canonical ordering, missing/unused
# call binding, and duplicate binding obligations.
python3 - "$T/canonical.witness" "$T/mutations" <<'PY'
from pathlib import Path
import struct
import sys

raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2]); out.mkdir()
bindings = 72 + 2 * 36

def put(name, value):
    out.joinpath(name + ".witness").write_bytes(value)

target = bytearray(raw)
struct.pack_into("<I", target, bindings + 28 + 20, 4)
put("target", target)

source_span = bytearray(raw)
struct.pack_into("<I", source_span, bindings + 28 + 12,
                 struct.unpack_from("<I", source_span, bindings + 28 + 12)[0] + 1)
put("span", source_span)

order = bytearray(raw)
left = bytes(order[bindings + 28:bindings + 56])
right = bytes(order[bindings + 56:bindings + 84])
order[bindings + 28:bindings + 56] = right
order[bindings + 56:bindings + 84] = left
put("order", order)

unused = bytearray(raw)
unused[bindings + 28 + 8] = 2
unused[bindings + 28 + 9] = 1
struct.pack_into("<I", unused, bindings + 28 + 20, 0)
put("unused", unused)

duplicate = bytearray(raw)
duplicate[bindings + 84 + 4:bindings + 112] = raw[bindings + 28 + 4:bindings + 56]
put("duplicate", duplicate)
PY

for NAME in target span order unused duplicate; do
  pack "mutation-$NAME" "$T/fixtures/valid.omgc" "$T/mutations/$NAME.witness"
  run_expect "$T/check" "$T/mutation-$NAME.rfn" 251 "$NAME role-3 mutation"
done

# Source-side target and module/owner controls are paired with a known-good
# witness so this layer, rather than producer rejection, must close them.
for NAME in missing wrong-owner private-cross-module; do
  pack "$NAME" "$T/fixtures/$NAME.omgc" "$T/canonical.witness"
  run_expect "$T/check" "$T/$NAME.rfn" 251 "$NAME source/witness relation"
done

# Producer controls remain aligned with the source checker's independently
# enforced relation and publish no witness on rejection.
for NAME in missing wrong-owner private-cross-module; do
  run_expect "$T/resolver" "$T/fixtures/$NAME.omgc" 251 "resolver $NAME"
done

# OMGRFN2 and OMGRFN3 are deliberately cross-rejected despite retaining the
# same component offsets.
python3 "$OLD_PACKER" "$T/fixtures/valid.omgc" "$T/canonical.witness" \
  "$T/ckir" "$T/elf" --result 70 > "$T/old-frame.rfn"
run_expect "$T/check" "$T/old-frame.rfn" 251 "OMGRFN2 frame cross-rejection"

python3 - "$T/canonical.rfn" "$T/frame-cases" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2]); out.mkdir()
bad = bytearray(raw); bad[8] = 2
out.joinpath("bad-version.rfn").write_bytes(bad)
over = bytearray(raw); struct.pack_into("<I", over, 20, 524289)
out.joinpath("witness-over.rfn").write_bytes(over)
PY
run_expect "$T/check" "$T/frame-cases/bad-version.rfn" 251 "wrong OMGRFN3 version"
run_expect "$T/check" "$T/frame-cases/witness-over.rfn" 252 "declared witness exhaustion"

# Later components are physically present but semantically opaque to layer 2.
printf changed-ckir > "$T/opaque.ckir"
printf changed-elf > "$T/opaque.elf"
python3 "$PACKER" "$T/fixtures/valid.omgc" "$T/canonical.witness" \
  "$T/opaque.ckir" "$T/opaque.elf" --result 70 > "$T/opaque.rfn"
run_expect "$T/check" "$T/opaque.rfn" 0 "opaque CKIR2/ELF components"

ELAPSED=$(($(date +%s) - STARTED))
echo "OMGRFN3 layer 2: independent same-module cross-source tokens, exact role-3 targets/spans/order/consumption, all unchanged OMGRSW1 rows, renamed controls, frame cross-rejection, and opaque later components passed below Delta (${ELAPSED}s; ${PROCEDURES}/128 procedures)"
