#!/usr/bin/env sh
# Focused first-artifact OMGRFN2 layer-2 source -> OMGRSW1 refinement gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN2 layer 2 first artifact: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN2 layer 2 first artifact: skipped ($TOOL absent)"
    exit 0
  }
done

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2-source-witness-independent.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2_bundle.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
for REQUIRED in "$CHECKER" "$PACKER" "$FIXTURE" "$RESOLVER"; do
  [ -f "$REQUIRED" ] || { echo "OMGRFN2 layer 2 first artifact: missing $REQUIRED" >&2; exit 1; }
done

PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$CHECKER")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN2 layer 2 first artifact: checker exceeds 128 procedures ($PROCEDURES)" >&2
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
printf '\nproc main() { return omgrfn2_l2_check() }\n' >> "$T/check.beta"
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null

python3 "$FIXTURE" build "$T/canonical"
mkdir "$T/controls"
cp "$T/canonical/compilation-envelope.bin" "$T/controls/canonical.omgc"

# These independently constructed valid compilations vary every name-bearing
# relation, custody labels, semantic package/source order, and the imported
# alias's row position.  They are not mutations of resolver-produced witness
# bytes and the checker contains no fixture identifiers.
python3 - "$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES" "$T/controls" <<'PY'
from pathlib import Path
import sys

sys.path.insert(0, sys.argv[1])
from resolution_handoff_reference import encode

out = Path(sys.argv[2])

def manifest(dep_key, root_key, dep_label, root_label, dep_module, root_module,
             alias, owner, machine, extras=()):
    aliases = list(extras) + [{"requester": root_key, "alias": alias, "target": dep_key}]
    return {
        "target": "linux_x86_64",
        "packages": [
            {"key": dep_key, "sources": [{"label": dep_label, "module": dep_module}]},
            {"key": root_key, "sources": [{"label": root_label, "module": root_module}]},
        ],
        "aliases": aliases,
        "root": {"package": root_key, "source": root_label, "owner": owner, "machine": machine},
    }

def write(name, dep_key, root_key, dep_label, root_label, dep_module, root_module,
          alias, record, owner, machine, extras=()):
    dep = f"module {dep_module};\npub data {record} [copy] {{ left: u8; right: u8; }}\n"
    root = (f"module {root_module};\nuse {alias}::{dep_module}::{record};\n"
            f"data {owner} {{ payload: {record}; }}\n"
            f"machine {owner}::{machine}(&mut self) -> u8 {{ self.payload.left }}\n")
    data = encode([(dep_label, dep), (root_label, root)],
                  manifest(dep_key, root_key, dep_label, root_label, dep_module,
                           root_module, alias, owner, machine, extras))
    out.joinpath(name + ".omgc").write_bytes(data)

k11, k22, k33 = "11"*32, "22"*32, "33"*32
write("renamed", k11, k22, "vendor/shape.omg", "program/start.omg",
      "geometry", "runner", "vendor", "Duo", "Vault", "execute")
write("reversed-order", k22, k11, "z-dep.omg", "a-root.omg",
      "types", "launch", "library", "Cell", "Driver", "start")
write("custody-labels", k11, k22, "aaa.omg", "zzz.omg",
      "geometry", "runner", "vendor", "Duo", "Vault", "execute")
write("alias-row-order", k11, k22, "dep.omg", "root.omg",
      "geometry", "runner", "zed", "Duo", "Vault", "execute",
      extras=({"requester": k22, "alias": "aaa", "target": k11},))
write("alias-module-ambiguity", k11, k22, "dep.omg", "root.omg",
      "geometry", "runner", "runner", "Duo", "Vault", "execute")
write("import-local-collision", k11, k22, "dep.omg", "root.omg",
      "geometry", "runner", "vendor", "Duo", "Duo", "execute")

# Structural envelopes whose source/alias relations must reject before any
# producer witness is trusted.
dep = "module geometry; pub data Duo [copy] { left: u8; right: u8; }\n"
root = ("module runner; use vendor::geometry::Duo; data Vault { payload: Duo; } "
        "machine Vault::execute(&mut self) -> u8 { self.payload.left }\n")
undeclared_alias = manifest(k11, k22, "dep.omg", "root.omg", "geometry", "runner",
                            "unused", "Vault", "execute")
out.joinpath("undeclared-alias.omgc").write_bytes(
    encode([("dep.omg", dep), ("root.omg", root)], undeclared_alias))
private_dep = "module geometry; data Duo [copy] { left: u8; right: u8; }\n"
out.joinpath("private-import.omgc").write_bytes(
    encode([("dep.omg", private_dep), ("root.omg", root)],
           manifest(k11, k22, "dep.omg", "root.omg", "geometry", "runner",
                    "vendor", "Vault", "execute")))

# Structurally valid OMGCOMP whose source disagrees with the resolver-owned
# module.  A resolver witness cannot be produced; pair it with pinned witness
# bytes below to isolate layer 2's source/module relation.
bad_manifest = manifest(k11, k22, "dep.omg", "root.omg", "claimed", "runner",
                        "vendor", "Vault", "execute")
bad_dep = "module authored; pub data Duo [copy] { left: u8; right: u8; }\n"
bad_root = ("module runner; use vendor::claimed::Duo; data Vault { payload: Duo; } "
            "machine Vault::execute(&mut self) -> u8 { self.payload.left }\n")
out.joinpath("module-mismatch.omgc").write_bytes(
    encode([("dep.omg", bad_dep), ("root.omg", bad_root)], bad_manifest))

long_dep = "module geometry; pub data Duo [copy] { left: u8; right: u8; }\n"
long_root = ("module runner; use vendor::geometry::Duo; data Vault { "
             + "x"*65 + ": Duo; } machine Vault::execute(&mut self) -> u8 { 0 }\n")
long_manifest = manifest(k11, k22, "dep.omg", "root.omg", "geometry", "runner",
                         "vendor", "Vault", "execute")
exact_root = ("module runner; use vendor::geometry::Duo; data Vault { "
              + "x"*64 + ": Duo; } machine Vault::execute(&mut self) -> u8 { 0 }\n")
out.joinpath("ident-64.omgc").write_bytes(
    encode([("dep.omg", long_dep), ("root.omg", exact_root)], long_manifest))
out.joinpath("long-ident.omgc").write_bytes(
    encode([("dep.omg", long_dep), ("root.omg", long_root)], long_manifest))
PY

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
    echo "OMGRFN2 layer 2 first artifact: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN2 layer 2 first artifact: $LABEL published stdout" >&2
    exit 1
  }
}

build_witness() {
  NAME=$1
  "$T/resolver" < "$T/controls/$NAME.omgc" > "$T/$NAME.witness"
  printf x > "$T/$NAME.ckir"
  printf x > "$T/$NAME.elf"
  python3 "$PACKER" "$T/controls/$NAME.omgc" "$T/$NAME.witness" \
    "$T/$NAME.ckir" "$T/$NAME.elf" --result 70 > "$T/$NAME.rfn"
}

for NAME in canonical renamed reversed-order custody-labels alias-row-order ident-64; do
  build_witness "$NAME"
  run_expect "$T/check" "$T/$NAME.rfn" 0 "valid $NAME source/witness"
done

# Custody labels do not enter identity or the witness.  Cross-pairing the same
# logical source/manifest with changed labels proves that relation explicitly.
cmp "$T/renamed.witness" "$T/custody-labels.witness" >/dev/null
python3 "$PACKER" "$T/controls/custody-labels.omgc" "$T/renamed.witness" \
  "$T/canonical.ckir" "$T/canonical.elf" --result 70 > "$T/custody-cross.rfn"
run_expect "$T/check" "$T/custody-cross.rfn" 0 "custody-label-invariant cross-pair"

# The canonical positive is independently pinned byte-for-byte.  This both
# enumerates every expected row/reserved byte and prevents a common producer
# bug from blessing its own output.  The SHA is printed only as a readable
# audit label; acceptance uses the literal bytes.
python3 - "$T/pinned.witness" "$T/canonical.witness" <<'PY'
from pathlib import Path
import base64
import hashlib
import sys

expected = base64.b64decode(
    "T01HUlNXMQABAAAAAABIAIwCAAACAAAAAQAAAAIAAAADAAAABQAAAAIAAAADAAAAAQAAAAAAAAABAAAA"
    "AAAAAAAAAAAAAAAAAAAAAAAAAAADAAAABwAAAAUAAAAAAAAAAAAAAAAAAAABAAAAAQAAAAEAAAABAAAA"
    "BwAAAAMAAAAAAAAAAQAAAAEAAAACAAAAAAAAAAEAAAAAAAAAEQAAABAAAAABAQAAAAAAAAAAAAADAAAA"
    "AAAAAB0AAAAEAAAAAAAAAAEAAAABAQAAOwAAAAQAAAAAAAAAAAAAAAEAAAABAAAAAgEAAEwAAAAFAAAA"
    "AQAAAP////8AAAAAAQEAAAAAAAAAAAAAGAAAAAQAAAAAAAAAAQAAAAEAAAABAAAAAAAAACkAAAAFAAAA"
    "AQAAAAIAAAACAAAAAQAAAAEAAABTAAAAAwAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAAAAAABAAAA"
    "BAAAAAEAAAAAAAAAAAAAAAAAAAACAAAAAwAAAAAAAAAAAAAAAAAAAAEAAAADAAAAAgAAAAAAAAAAAAAA"
    "AAAAAP///38EAAAAAQAAAAAAAAAAAAAAAAAAAP8AAAAAAAAAAAAAAAAAAAAAAAAAAgAAAAEAAAABAAAA"
    "AQAAAAEAAAACAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAEAAAAKgAAAAUAAAABAAAAAAAAAAEAAAAEAAAA"
    "OQAAAAYAAAACAAAAAQAAAAAAAAAAAAAANQAAAAQAAAAAAAAAAgAAAAEAAAACAAAABAAAAAAAAAAAAAAA"
    "AAAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAgAAAG4AAACzAAAA/////wAAAAAAAAAAAAAAAA=="
)
if len(expected) != 652:
    raise SystemExit("pinned OMGRSW1 length drift")
if hashlib.sha256(expected).hexdigest() != "192dcad1ad1b281bd37dea8f8d68798c62ff2a8fe3db2dd45bd7e831f9e04d24":
    raise SystemExit("pinned OMGRSW1 digest drift")
actual = Path(sys.argv[2]).read_bytes()
if actual != expected:
    raise SystemExit("resolver output differs from independently pinned canonical rows")
Path(sys.argv[1]).write_bytes(expected)
PY
python3 "$PACKER" "$T/controls/canonical.omgc" "$T/pinned.witness" \
  "$T/canonical.ckir" "$T/canonical.elf" --result 70 > "$T/pinned.rfn"
run_expect "$T/check" "$T/pinned.rfn" 0 "independently pinned canonical rows"

python3 - "$T/pinned.rfn" "$T/frame-cases" <<'PY'
from pathlib import Path
import struct
import sys
raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2]); out.mkdir()
bad = bytearray(raw); bad[0] ^= 1
out.joinpath("bad-magic.rfn").write_bytes(bad)
over = bytearray(raw); struct.pack_into("<I", over, 20, 524289)
out.joinpath("witness-over.rfn").write_bytes(over)
PY
run_expect "$T/check" "$T/frame-cases/bad-magic.rfn" 251 "malformed frame"
run_expect "$T/check" "$T/frame-cases/witness-over.rfn" 252 "declared witness exhaustion"

# Every nonempty OMGRSW1 table is covered by a byte mutation.  Header-count
# drift covers both empty parameter tables; exact whole-byte comparison makes
# all unselected bytes and reserved fields equally binding.
python3 - "$T/pinned.witness" "$T/mutated" <<'PY'
from pathlib import Path
import struct
import sys

raw = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2]); out.mkdir()
counts = struct.unpack_from("<11I", raw, 20)
strides = (36,48,28,28,24,24,24,40,24,40,24)
at = 72
for table, (count, stride) in enumerate(zip(counts, strides)):
    if count:
        changed = bytearray(raw)
        changed[at + min(stride - 1, 7)] ^= 1
        out.joinpath(f"table-{table}.witness").write_bytes(changed)
    at += count * stride
changed = bytearray(raw); changed[20] ^= 1
out.joinpath("header-count.witness").write_bytes(changed)
PY
for MUTATED in "$T/mutated"/*.witness; do
  NAME=$(basename "$MUTATED" .witness)
  python3 "$PACKER" "$T/controls/canonical.omgc" "$MUTATED" \
    "$T/canonical.ckir" "$T/canonical.elf" --result 70 > "$T/mutated-$NAME.rfn"
  run_expect "$T/check" "$T/mutated-$NAME.rfn" 251 "witness row mutation $NAME"
done

# Both sides of each cross-pair are independently valid.  Reversed semantic
# source order changes nominal record/type/declaration IDs despite identical
# record shapes; name/custody changes alter spans and bindings.
for PAIR in "canonical reversed-order" "reversed-order canonical" \
  "canonical renamed" "renamed canonical"; do
  set -- $PAIR
  SOURCE=$1
  WITNESS=$2
  python3 "$PACKER" "$T/controls/$SOURCE.omgc" "$T/$WITNESS.witness" \
    "$T/canonical.ckir" "$T/canonical.elf" --result 70 > "$T/cross-$SOURCE-$WITNESS.rfn"
  run_expect "$T/check" "$T/cross-$SOURCE-$WITNESS.rfn" 251 \
    "valid source/witness cross-pair $SOURCE/$WITNESS"
done

python3 "$PACKER" "$T/controls/module-mismatch.omgc" "$T/pinned.witness" \
  "$T/canonical.ckir" "$T/canonical.elf" --result 70 > "$T/module-mismatch.rfn"
run_expect "$T/check" "$T/module-mismatch.rfn" 251 "authored/owned module mismatch"

for NAME in alias-module-ambiguity import-local-collision undeclared-alias \
  private-import; do
  run_expect "$T/resolver" "$T/controls/$NAME.omgc" 251 \
    "$NAME producer-side semantic rejection"
  python3 "$PACKER" "$T/controls/$NAME.omgc" "$T/pinned.witness" \
    "$T/canonical.ckir" "$T/canonical.elf" --result 70 > "$T/$NAME.rfn"
  run_expect "$T/check" "$T/$NAME.rfn" 251 "$NAME resolution rejection"
done

python3 "$PACKER" "$T/controls/long-ident.omgc" "$T/pinned.witness" \
  "$T/canonical.ckir" "$T/canonical.elf" --result 70 > "$T/long-ident.rfn"
run_expect "$T/check" "$T/long-ident.rfn" 252 "source-token resource exhaustion"

# CKIR and ELF are opaque to this layer by construction.
printf changed-ckir > "$T/opaque.ckir"
printf changed-elf > "$T/opaque.elf"
python3 "$PACKER" "$T/controls/canonical.omgc" "$T/pinned.witness" \
  "$T/opaque.ckir" "$T/opaque.elf" --result 70 > "$T/opaque.rfn"
run_expect "$T/check" "$T/opaque.rfn" 0 "opaque CKIR/ELF components"

ELAPSED=$(($(date +%s) - STARTED))
echo "OMGRFN2 layer 2 first artifact: independent source tokens, alias/module/declaration joins, every OMGRSW1 row, pinned canonical bytes, renamed/reordered/alias/custody controls, nominal-ID cross-pairs, and opaque later components passed below Delta (${ELAPSED}s; ${PROCEDURES}/128 procedures)"
