#!/usr/bin/env sh
# Structural OMGCOMP checker gate.  The native checker owns the broad relation
# matrix; the lowermachine-built checker repeats one 0/251/252 observation.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || {
      echo "compilation envelope: repository root not found" >&2
      exit 2
    }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" || exit $?

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "compilation envelope: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "compilation envelope: skipped ($TOOL absent)"
    exit 0
  }
done

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-compilation-check.alp"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_compilation.py"
[ -f "$CHECKER" ] || { echo "compilation envelope: checker absent" >&2; exit 1; }
[ -f "$PACKER" ] || { echo "compilation envelope: packer absent" >&2; exit 1; }

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$CHECKER" "$T/checker.native" >/dev/null
DELTA_ARCH=aarch64 "$OMEGA_PATH_DELTA_RUST/target/debug/delta" \
  "$OMEGA_PATH_DELTA/samples/lowermachine.alp" "$T/lowermachine" >/dev/null

python3 - "$T/lowermachine" "$CHECKER" "$T/checker.self.s" <<'PY'
import subprocess, sys
lowermachine, source, output = sys.argv[1:]
with open(source, "rb") as stdin, open(output, "wb") as stdout:
    try:
        result = subprocess.run(
            [lowermachine], stdin=stdin, stdout=stdout,
            stderr=subprocess.PIPE, timeout=60, check=False,
        )
    except subprocess.TimeoutExpired:
        raise SystemExit("compilation envelope: lowermachine exceeded 60 seconds")
if result.returncode != 0:
    raise SystemExit(
        "compilation envelope: lowermachine failed: "
        + result.stderr.decode("utf-8", errors="replace")[:240]
    )
PY
clang -arch arm64 -o "$T/checker.self" "$T/checker.self.s"
codesign -f -s - "$T/checker.self" >/dev/null 2>&1

mkdir "$T/cases"
PYTHONPATH="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER" python3 - "$T/cases" <<'PY'
from pathlib import Path
import struct, sys
import omega_bootstrap_bundle as b
import omega_bootstrap_compilation as c

out = Path(sys.argv[1])
rows = []

def key(n): return f"{n:064x}"
def package(n, sources):
    return {"key": key(n), "sources": [{"label": x, "module": y} for x, y in sources]}
def manifest(packages, aliases, root=(1, "a/root.omg", "Owner", "main")):
    return {
        "target": "linux_x86_64", "packages": packages,
        "aliases": [{"requester": key(r), "alias": a, "target": key(t)} for r, a, t in aliases],
        "root": {"package": key(root[0]), "source": root[1], "owner": root[2], "machine": root[3]},
    }
def bundle(entries): return b.encode([b.Entry(label, content) for label, content in entries])
def add(name, status, data):
    path = out / (name + ".omgc")
    path.write_bytes(data); rows.append((name, status, path))
def u32(data, at, value):
    result = bytearray(data); struct.pack_into("<I", result, at, value); return bytes(result)
def bytes_at(data, at, value):
    result = bytearray(data); result[at:at + len(value)] = value; return bytes(result)
def layout(data):
    h = c.HEADER.unpack_from(data); p = 64; s = p + h[9] * 48
    a = s + h[10] * 20; strings = a + h[11] * 16
    return h, p, s, a, strings, strings + h[7]
def string_rows(data):
    h, _, _, _, pos, end = layout(data); result = {}
    while pos < end:
        n = struct.unpack_from("<I", data, pos)[0]
        result[data[pos + 4:pos + 4 + n].decode("ascii")] = (pos, n)
        pos += 4 + n
    return result

basic_bundle = bundle([
    ("c/other.omg", b"data Other {}"),
    ("a/root.omg", b"machine Owner::main(&mut self) {}"),
    ("b/lib.omg", b"data Lib {}\x00"),
])
basic_manifest = manifest(
    [package(3, [("c/other.omg", "other")]), package(1, [("a/root.omg", "")]), package(2, [("b/lib.omg", "lib")])],
    [(1, "extra", 3), (2, "leaf", 3), (1, "dep", 2)],
)
good = c.encode_manifest(basic_manifest, basic_bundle)
h, packages, sources, aliases, strings, bundle_at = layout(good)
sr = string_rows(good)
add("canonical-three-package", 0, good)

# A single canonical string can satisfy both module-path and alias roles.
shared_manifest = manifest(
    [package(1, [("a/root.omg", "dep")]), package(2, [("b/lib.omg", "lib")]), package(3, [("c/other.omg", "other")])],
    [(1, "dep", 2), (1, "extra", 3), (2, "leaf", 3)],
)
add("canonical-role-intersection", 0, c.encode_manifest(shared_manifest, basic_bundle))

# Header/extent relations.
add("reject-magic", 251, bytes_at(good, 0, b"X"))
add("reject-schema", 251, bytes_at(good, 8, b"\x02"))
add("reject-total-relation", 251, u32(good, 16, len(good) - 1))
add("reject-trailing-eof", 251, good + b"x")
add("reject-signed-overflow", 251, u32(good, 24, 0xFFFFFFFF))
add("reject-string-extent-arithmetic", 251, u32(good, 24, 0x7FFFFFFF))

# Package/source/key/span/permutation relations.
add("reject-package-dense", 251, u32(good, packages, 1))
add("reject-zero-package-key", 251, bytes_at(good, packages + 4, bytes(32)))
swapped_keys = bytearray(good)
swapped_keys[packages + 4:packages + 36], swapped_keys[packages + 52:packages + 84] = \
    swapped_keys[packages + 52:packages + 84], swapped_keys[packages + 4:packages + 36]
add("reject-package-key-order", 251, bytes(swapped_keys))
add("reject-empty-package-span", 251, u32(good, packages + 40, 0))
add("reject-package-partition", 251, u32(good, packages + 36, 1))
add("reject-source-dense", 251, u32(good, sources, 1))
add("reject-source-owner", 251, u32(good, sources + 4, 1))
add("reject-bundle-permutation", 251, u32(good, sources + 20 + 8, 0))
add("reject-module-string-range", 251, u32(good, sources + 12, h[8]))
add("reject-source-flags", 251, u32(good, sources + 16, 1))
add("reject-root-source-owner", 251, u32(good, 48, 1))

# Alias order/grammar/graph relations.
add("reject-alias-reserved", 251, u32(good, aliases + 12, 1))
add("reject-alias-self", 251, u32(good, aliases + 8, 0))
alias_disorder = bytearray(good)
alias_disorder[aliases:aliases + 16], alias_disorder[aliases + 16:aliases + 32] = \
    alias_disorder[aliases + 16:aliases + 32], alias_disorder[aliases:aliases + 16]
add("reject-alias-order", 251, bytes(alias_disorder))
dep_at, dep_len = sr["dep"]
add("reject-alias-snake-case", 251, bytes_at(good, dep_at + 4, b"d__"))
add("reject-alias-cycle", 251, u32(good, aliases + 32 + 8, 0))
unreachable = u32(u32(good, aliases + 8, 2), aliases + 16 + 8, 2)
add("reject-unreachable-package", 251, unreachable)

# String table order/use/ASCII/role relations.
owner_at, owner_len = sr["Owner"]
add("reject-string-nonascii", 251, bytes_at(good, owner_at + 4, b"\xff"))
leaf_at, leaf_len = sr["leaf"]
add("reject-string-duplicate", 251, bytes_at(good, leaf_at + 4, b"main"))
other_id = sorted(sr).index("other")
lib_id = sorted(sr).index("lib")
add("reject-unused-string", 251, u32(good, sources + 40 + 12, lib_id))
add("reject-root-identifier-role", 251, u32(good, 52, sorted(sr).index("")))

# Nested bundle framing, label grammar/order, source ordering, and exact EOF.
add("reject-bundle-magic", 251, bytes_at(good, bundle_at, b"X"))
add("reject-bundle-count", 251, u32(good, bundle_at + 12, 2))
first_label = bundle_at + 24
add("reject-label-grammar", 251, bytes_at(good, first_label, b"/"))
add("reject-label-order", 251, bytes_at(good, first_label, b"z"))

# Declared public resource selections.  Counts select exhaustion before the
# inconsistent fixed-table extent is inspected; actual length has its own
# independently observed preflight ceiling.
add("exhaust-package-count-17", 252, u32(good, 32, 17))
add("exhaust-source-count-17", 252, u32(good, 36, 17))
add("exhaust-alias-count-33", 252, u32(good, 40, 33))
add("exhaust-string-count-65", 252, u32(good, 28, 65))
add("exhaust-bundle-length-263313", 252, u32(good, 20, 263313))
add("exhaust-envelope-length-267281", 252, bytes(267281))

# Exact/adjacent component and bundle-label ceilings use otherwise canonical
# encodings so the 252 observation is not an extent-mismatch shortcut.
component_manifest = manifest([package(1, [("a/root.omg", "A" * 64)])], [], root=(1, "a/root.omg", "Owner", "main"))
component_bundle = bundle([("a/root.omg", b"")])
component_exact = c.encode_manifest(component_manifest, component_bundle)
add("limit-identifier-component-64", 0, component_exact)
component_rows = string_rows(component_exact)
at, old = component_rows["A" * 64]
component_over = bytearray(component_exact)
component_over[at:at + 4] = struct.pack("<I", 65)
component_over[at + 4:at + 4 + old] = b"A" * 65
struct.pack_into("<I", component_over, 16, len(component_over))
struct.pack_into("<I", component_over, 24, c.HEADER.unpack_from(component_exact)[7] + 1)
add("exhaust-identifier-component-65", 252, bytes(component_over))

label64 = "a" * 64
label_bundle = bundle([(label64, b"")])
label_manifest = manifest([package(1, [(label64, "")])], [], root=(1, label64, "Owner", "main"))
label_exact = c.encode_manifest(label_manifest, label_bundle)
add("limit-bundle-label-64", 0, label_exact)
lh, _, _, _, _, lb = layout(label_exact)
label_over = bytearray(label_exact)
label_len_at = lb + 16
struct.pack_into("<I", label_over, label_len_at, 65)
label_over[label_len_at + 8:label_len_at + 8 + 64] = b"a" * 65
struct.pack_into("<I", label_over, 16, len(label_over))
struct.pack_into("<I", label_over, 20, lh[6] + 1)
add("exhaust-bundle-label-65", 252, bytes(label_over))

# Canonical realizable public-boundary fixtures.  These are deliberately
# generated independently of the header-only adjacent teeth above.
def sources_for(count, prefix="s"):
    entries = [(f"{prefix}{i:02d}.omg", b"") for i in range(count)]
    specs = [(label, "") for label, _ in entries]
    return entries, specs

entries16, specs16 = sources_for(16)
bundle16 = bundle(entries16)
packages16 = [package(i + 1, [specs16[i]]) for i in range(16)]
aliases15 = [(1, f"p{i:02d}", i + 1) for i in range(1, 16)]
exact_package16 = c.encode_manifest(
    manifest(packages16, aliases15, root=(1, specs16[0][0], "Owner", "main")), bundle16
)
add("limit-package-count-16", 0, exact_package16)
add("limit-source-count-16", 0, exact_package16)

alias_entries = [("a.omg", b""), ("b.omg", b"")]
alias_specs = [(1, f"alias{i:02d}", 2) for i in range(32)]
exact_alias32 = c.encode_manifest(
    manifest([package(1, [("a.omg", "")]), package(2, [("b.omg", "")])], alias_specs,
             root=(1, "a.omg", "Owner", "main")), bundle(alias_entries)
)
add("limit-alias-count-32", 0, exact_alias32)

content_exact_bundle = bundle([("main.omg", b"x" * 131072)])
content_manifest = manifest([package(1, [("main.omg", "")])], [], root=(1, "main.omg", "Owner", "main"))
content_exact = c.encode_manifest(content_manifest, content_exact_bundle)
add("limit-source-content-131072", 0, content_exact)
ch, _, _, _, _, cb = layout(content_exact)
content_over = bytearray(content_exact)
content_len_at = cb + 20
struct.pack_into("<I", content_over, content_len_at, 131073)
content_over.append(120)
struct.pack_into("<I", content_over, 16, len(content_over))
struct.pack_into("<I", content_over, 20, ch[6] + 1)
add("exhaust-source-content-131073", 252, bytes(content_over))

aggregate_bundle = bundle([("a.omg", b"x" * 131072), ("b.omg", b"y" * 131072), ("c.omg", b"")])
aggregate_manifest = manifest(
    [package(1, [("a.omg", ""), ("b.omg", ""), ("c.omg", "")])], [],
    root=(1, "a.omg", "Owner", "main"),
)
aggregate_exact = c.encode_manifest(aggregate_manifest, aggregate_bundle)
add("limit-aggregate-content-262144", 0, aggregate_exact)
ah, _, aggregate_sources, _, _, ab = layout(aggregate_exact)
source_disorder = u32(u32(aggregate_exact, aggregate_sources + 8, 1),
                      aggregate_sources + 20 + 8, 0)
add("reject-source-label-order", 251, source_disorder)
# Third entry begins after header, two entry headers/labels/maximal contents.
third = ab + 16 + (8 + 5 + 131072) * 2
aggregate_over = bytearray(aggregate_exact)
struct.pack_into("<I", aggregate_over, third + 4, 1)
aggregate_over.append(122)
struct.pack_into("<I", aggregate_over, 16, len(aggregate_over))
struct.pack_into("<I", aggregate_over, 20, ah[6] + 1)
add("exhaust-aggregate-content-262145", 252, bytes(aggregate_over))

max_labels = [f"{i:02d}" + "a" * 62 for i in range(16)]
max_bundle = bundle([(label, b"x" * 131072 if i < 2 else b"") for i, label in enumerate(max_labels)])
max_bundle_manifest = manifest(
    [package(1, [(label, "") for label in max_labels])], [],
    root=(1, max_labels[0], "Owner", "main"),
)
max_bundle_envelope = c.encode_manifest(max_bundle_manifest, max_bundle)
assert len(max_bundle) == 263312
add("limit-aggregate-labels-1024", 0, max_bundle_envelope)
add("limit-bundle-bytes-263312", 0, max_bundle_envelope)

def path_of_length(length, number):
    # First component carries a stable distinct prefix; later components make
    # every requested length through the eight-component/64-byte grammar.
    parts = []
    remaining = length
    first = f"z{number:02d}"
    take = min(64, remaining)
    if take < len(first): return first[:take]
    parts.append(first + "a" * (take - len(first))); remaining -= take
    while remaining:
        remaining -= 2
        take = min(64, remaining)
        parts.append("b" * take); remaining -= take
    return "::".join(parts)

payload_lengths = [4] * 16
base_payload = sum(payload_lengths) + len("Owner") + len("main")
needed = 2048 - base_payload
for idx in range(16):
    grow = min(522, needed)
    payload_lengths[idx] += grow; needed -= grow
assert needed == 0
payload_modules = [path_of_length(length, i) for i, length in enumerate(payload_lengths)]
payload_entries = [(f"p{i:02d}.omg", b"") for i in range(16)]
payload_manifest = manifest(
    [package(1, [(label, payload_modules[i]) for i, (label, _) in enumerate(payload_entries)])], [],
    root=(1, payload_entries[0][0], "Owner", "main"),
)
payload_exact = c.encode_manifest(payload_manifest, bundle(payload_entries))
assert sum(len(value) for value in c.decode(payload_exact).strings) == 2048
add("limit-string-payload-2048", 0, payload_exact)
payload_map = string_rows(payload_exact)
target = next(value for value in payload_modules if len(value.split("::")[-1]) < 64)
pat, plen = payload_map[target]
payload_over = bytearray(payload_exact)
payload_over[pat:pat + 4] = struct.pack("<I", plen + 1)
payload_over[pat + 4:pat + 4 + plen] = target.encode() + b"a"
struct.pack_into("<I", payload_over, 16, len(payload_over))
struct.pack_into("<I", payload_over, 24, c.HEADER.unpack_from(payload_exact)[7] + 1)
add("exhaust-string-payload-2049", 252, bytes(payload_over))

depth8 = "::".join("a" for _ in range(8))
depth_manifest = manifest([package(1, [("main.omg", depth8)])], [], root=(1, "main.omg", "Owner", "main"))
depth_exact = c.encode_manifest(depth_manifest, bundle([("main.omg", b"")]))
add("limit-module-depth-8", 0, depth_exact)
depth_map = string_rows(depth_exact); dat, dlen = depth_map[depth8]
depth_over = bytearray(depth_exact)
depth_over[dat:dat + 4] = struct.pack("<I", dlen + 3)
depth_over[dat + 4:dat + 4 + dlen] = depth8.encode() + b"::a"
struct.pack_into("<I", depth_over, 16, len(depth_over))
struct.pack_into("<I", depth_over, 24, c.HEADER.unpack_from(depth_exact)[7] + 3)
add("exhaust-module-depth-9", 252, bytes(depth_over))

with (out / "manifest.tsv").open("w", encoding="utf-8") as f:
    for name, status, path in rows: f.write(f"{name}\t{status}\t{path}\n")
with (out / "self.tsv").open("w", encoding="utf-8") as f:
    for wanted in ("canonical-three-package", "reject-magic", "exhaust-package-count-17"):
        for name, status, path in rows:
            if name == wanted: f.write(f"{name}\t{status}\t{path}\n")
PY

python3 - "$T/checker.native" "$T/checker.self" "$T/cases/manifest.tsv" "$T/cases/self.tsv" <<'PY'
from pathlib import Path
import subprocess, sys, time
native, self_built, native_manifest, self_manifest = sys.argv[1:]

def run(exe, manifest, timeout):
    total = 0.0
    for row in Path(manifest).read_text().splitlines():
        name, expected, path = row.split("\t"); expected = int(expected)
        started = time.monotonic()
        try:
            with open(path, "rb") as source:
                result = subprocess.run([exe], stdin=source, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, timeout=timeout, check=False)
        except subprocess.TimeoutExpired:
            raise SystemExit(f"compilation envelope FAIL - {name} exceeded {timeout}s")
        elapsed = time.monotonic() - started; total += elapsed
        if result.returncode != expected or result.stdout:
            raise SystemExit(
                f"compilation envelope FAIL - {name}: got "
                f"{result.returncode}/{len(result.stdout)}B, expected {expected}/0B"
            )
    return total

native_time = run(native, native_manifest, 5)
self_time = run(self_built, self_manifest, 20)
print(f"compilation envelope: PASS native matrix in {native_time:.2f}s")
print(f"compilation envelope: PASS self-built 0/251/252 in {self_time:.2f}s")
print("compilation envelope: NOTE string-count 64 is canonically unreachable (at most 50 role references)")
print("compilation envelope: NOTE aggregate-label 1025 cannot precede the 16x64 component ceiling")
print("compilation envelope: NOTE total-envelope 267280 is a conservative preflight bound, not a realizable canonical maximum")
PY
