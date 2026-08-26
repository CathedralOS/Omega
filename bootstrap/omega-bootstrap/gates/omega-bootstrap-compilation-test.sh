#!/usr/bin/env sh
set -eu
GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ -z "${OMEGA_REPO_ROOT:-}" ]; then
  OMEGA_REPO_ROOT=$GATE_DIR
  while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
    PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
    [ "$PARENT" != "$OMEGA_REPO_ROOT" ] \
      || { echo "omega-bootstrap compilation: repository root not found" >&2; exit 2; }
    OMEGA_REPO_ROOT=$PARENT
  done
fi
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh" || exit $?
command -v python3 >/dev/null 2>&1 \
  || { echo "omega-bootstrap compilation: skipped (python3 absent)"; exit 0; }

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
export OMEGA_COMPILATION_TOOL="$OMEGA_PATH_OMEGA_BOOTSTRAP/source/compiler/omega/omega_bootstrap_compilation.py"
export OMEGA_COMPILATION_TMP="$T"

python3 - <<'PY'
from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
import struct
import subprocess
import sys
from pathlib import Path

tool_path = Path(os.environ["OMEGA_COMPILATION_TOOL"])
tmp = Path(os.environ["OMEGA_COMPILATION_TMP"])
sys.path.insert(0, str(tool_path.parent))
spec = importlib.util.spec_from_file_location("omega_bootstrap_compilation", tool_path)
assert spec is not None and spec.loader is not None
c = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = c
spec.loader.exec_module(c)
import omega_bootstrap_bundle as b


def fail(message: str) -> None:
    raise AssertionError(message)


def expect_status(name: str, status: int, action) -> None:
    try:
        action()
    except c.CompilationError as error:
        if error.status != status:
            fail(f"{name}: expected {status}, got {error.status}: {error}")
    else:
        fail(f"{name}: unexpectedly accepted")


def key(number: int) -> str:
    return f"{number:064x}"


def bundle(entries: list[tuple[str, bytes]]) -> bytes:
    return b.encode([b.Entry(label, content) for label, content in entries])


def package(number: int, sources: list[tuple[str, str]]) -> dict:
    return {
        "key": key(number),
        "sources": [{"label": label, "module": module} for label, module in sources],
    }


def manifest(packages: list[dict], aliases: list[tuple[int, str, int]], root=(1, "a/root.omg", "Owner", "main")) -> dict:
    return {
        "target": "linux_x86_64",
        "packages": packages,
        "aliases": [
            {"requester": key(requester), "alias": alias, "target": key(target)}
            for requester, alias, target in aliases
        ],
        "root": {"package": key(root[0]), "source": root[1], "owner": root[2], "machine": root[3]},
    }


def replace_u16(data: bytes, offset: int, value: int) -> bytes:
    result = bytearray(data)
    struct.pack_into("<H", result, offset, value)
    return bytes(result)


def replace_u32(data: bytes, offset: int, value: int) -> bytes:
    result = bytearray(data)
    struct.pack_into("<I", result, offset, value)
    return bytes(result)


def layout(data: bytes) -> tuple[int, int, int, int]:
    fields = c.HEADER.unpack_from(data)
    bundle_length, string_length = fields[6], fields[7]
    packages, sources, aliases = fields[9], fields[10], fields[11]
    package_at = c.HEADER.size
    source_at = package_at + packages * c.PACKAGE_ROW.size
    alias_at = source_at + sources * c.SOURCE_ROW.size
    string_at = alias_at + aliases * c.ALIAS_ROW.size
    bundle_at = string_at + string_length
    assert len(data) - bundle_at == bundle_length
    return package_at, source_at, alias_at, string_at


def string_rows(data: bytes) -> tuple[dict[str, tuple[int, int]], int]:
    _, _, _, cursor = layout(data)
    string_length = c.HEADER.unpack_from(data)[7]
    end = cursor + string_length
    rows = {}
    while cursor < end:
        length = struct.unpack_from("<I", data, cursor)[0]
        payload = cursor + 4
        rows[data[payload : payload + length].decode("ascii")] = (payload, length)
        cursor = payload + length
    return rows, end


basic_bundle = bundle(
    [
        ("c/other.omg", b"data Other {}"),
        ("a/root.omg", b"machine Owner::main(&mut self) {}"),
        ("b/lib.omg", b"data Lib {}\x00"),
    ]
)
basic_manifest = manifest(
    [package(3, [("c/other.omg", "other")]), package(1, [("a/root.omg", "")]), package(2, [("b/lib.omg", "lib")])],
    [(1, "extra", 3), (2, "leaf", 3), (1, "dep", 2)],
)
encoded = c.encode_manifest(basic_manifest, basic_bundle)
decoded = c.decode(encoded)
view = c.inspect(decoded)
assert [row["key"] for row in view["packages"]] == [key(1), key(2), key(3)]
assert [row["label"] for row in view["sources"]] == ["a/root.omg", "b/lib.omg", "c/other.omg"]
assert [(row["requester"], row["alias"]) for row in view["aliases"]] == [(0, "dep"), (0, "extra"), (1, "leaf")]
assert view["root"] == {"package": 0, "source": 0, "owner": "Owner", "machine": "main"}
assert view["envelope_sha256"] == hashlib.sha256(encoded).hexdigest()

reordered = copy.deepcopy(basic_manifest)
reordered["packages"].reverse()
for item in reordered["packages"]:
    item["sources"].reverse()
reordered["aliases"].reverse()
assert c.encode_manifest(reordered, basic_bundle) == encoded

# Exercise all three public commands, including stdin verification and stable JSON inspection.
(tmp / "manifest.json").write_text(json.dumps(basic_manifest), encoding="utf-8")
(tmp / "sources.omg0b").write_bytes(basic_bundle)
packed = subprocess.run(
    [sys.executable, str(tool_path), "pack", str(tmp / "manifest.json"), str(tmp / "sources.omg0b")],
    check=True,
    stdout=subprocess.PIPE,
).stdout
assert packed == encoded
subprocess.run([sys.executable, str(tool_path), "verify"], input=packed, check=True)
inspected = subprocess.run(
    [sys.executable, str(tool_path), "inspect"], input=packed, check=True, stdout=subprocess.PIPE
).stdout
assert json.loads(inspected) == view

# Header and whole-extent malformed controls.
expect_status("truncated header", 251, lambda: c.decode(encoded[:63]))
wrong_magic = bytearray(encoded); wrong_magic[0] ^= 1
expect_status("magic", 251, lambda: c.decode(bytes(wrong_magic)))
for name, offset, value in (("major", 8, 2), ("minor", 10, 1), ("target", 12, 2), ("flags", 14, 1)):
    expect_status(name, 251, lambda o=offset, v=value: c.decode(replace_u16(encoded, o, v)))
for name, offset, value in (
    ("declared total mismatch", 16, len(encoded) + 1),
    ("bundle extent mismatch", 20, len(basic_bundle) + 1),
    ("string extent mismatch", 24, c.HEADER.unpack_from(encoded)[7] + 1),
    ("header reserved", 60, 1),
):
    expect_status(name, 251, lambda o=offset, v=value: c.decode(replace_u32(encoded, o, v)))
expect_status("trailing envelope byte", 251, lambda: c.decode(encoded + b"x"))
expect_status("overflow extent is malformed", 251, lambda: c.decode(replace_u32(encoded, 16, 0xFFFFFFFF)))
for name, offset in (("package count overflow", 32), ("source count overflow", 36), ("alias count overflow", 40), ("string count overflow", 28)):
    expect_status(name, 251, lambda o=offset: c.decode(replace_u32(encoded, o, 0xFFFFFFFF)))
for name, offset, value in (
    ("package count 17", 32, 17),
    ("source count 17", 36, 17),
    ("alias count 33", 40, 33),
    ("string count 65", 28, 65),
    ("bundle derived maximum adjacent", 20, c.MAX_BUNDLE_BYTES + 1),
    ("envelope derived maximum adjacent", 16, c.MAX_ENVELOPE_BYTES + 1),
):
    expect_status(name, 252, lambda o=offset, v=value: c.decode(replace_u32(encoded, o, v)))

package_at, source_at, alias_at, _ = layout(encoded)

# Package row canonicality, commitments, spans, and reserved fields.
expect_status("package dense ID", 251, lambda: c.decode(replace_u32(encoded, package_at, 1)))
zero_key = bytearray(encoded); zero_key[package_at + 4 : package_at + 36] = bytes(32)
expect_status("zero package key", 251, lambda: c.decode(bytes(zero_key)))
duplicate_key = bytearray(encoded)
duplicate_key[package_at + 48 + 4 : package_at + 48 + 36] = duplicate_key[package_at + 4 : package_at + 36]
expect_status("duplicate package key", 251, lambda: c.decode(bytes(duplicate_key)))
decreasing_key = bytearray(encoded)
decreasing_key[package_at + 4] = 4
expect_status("package key ordering", 251, lambda: c.decode(bytes(decreasing_key)))
expect_status("zero package source count", 251, lambda: c.decode(replace_u32(encoded, package_at + 40, 0)))
expect_status("package span start", 251, lambda: c.decode(replace_u32(encoded, package_at + 36, 1)))
expect_status("package span overflow", 251, lambda: c.decode(replace_u32(encoded, package_at + 40, 4)))
expect_status("package reserved", 251, lambda: c.decode(replace_u32(encoded, package_at + 44, 1)))

# Source density, ownership, bundle permutation/order, string IDs, and flags.
expect_status("source dense ID", 251, lambda: c.decode(replace_u32(encoded, source_at, 1)))
expect_status("source owner range", 251, lambda: c.decode(replace_u32(encoded, source_at + 4, 3)))
expect_status("source owner span", 251, lambda: c.decode(replace_u32(encoded, source_at + 4, 1)))
expect_status("bundle ID range", 251, lambda: c.decode(replace_u32(encoded, source_at + 8, 3)))
expect_status("bundle ID duplicate", 251, lambda: c.decode(replace_u32(encoded, source_at + 20 + 8, 0)))
expect_status("module string ID", 251, lambda: c.decode(replace_u32(encoded, source_at + 12, 99)))
expect_status("source flags", 251, lambda: c.decode(replace_u32(encoded, source_at + 16, 1)))

two_in_one_bundle = bundle([("a/root.omg", b"a"), ("a/z.omg", b"z")])
two_in_one = manifest([package(1, [("a/root.omg", ""), ("a/z.omg", "z")])], [], root=(1, "a/root.omg", "Owner", "main"))
two_encoded = c.encode_manifest(two_in_one, two_in_one_bundle)
_, two_source_at, _, _ = layout(two_encoded)
reversed_sources = bytearray(two_encoded)
first_bundle = struct.unpack_from("<I", reversed_sources, two_source_at + 8)[0]
second_bundle = struct.unpack_from("<I", reversed_sources, two_source_at + 20 + 8)[0]
struct.pack_into("<I", reversed_sources, two_source_at + 8, second_bundle)
struct.pack_into("<I", reversed_sources, two_source_at + 20 + 8, first_bundle)
expect_status("source label ordering", 251, lambda: c.decode(bytes(reversed_sources)))

# Alias row structure, canonical package spelling, ordering, graph, and reach.
expect_status("alias requester range", 251, lambda: c.decode(replace_u32(encoded, alias_at, 3)))
expect_status("alias string range", 251, lambda: c.decode(replace_u32(encoded, alias_at + 4, 99)))
expect_status("alias target range", 251, lambda: c.decode(replace_u32(encoded, alias_at + 8, 3)))
expect_status("alias self edge", 251, lambda: c.decode(replace_u32(encoded, alias_at + 8, 0)))
expect_status("alias reserved", 251, lambda: c.decode(replace_u32(encoded, alias_at + 12, 1)))
swapped_aliases = bytearray(encoded)
row0 = bytes(swapped_aliases[alias_at : alias_at + 16])
row1 = bytes(swapped_aliases[alias_at + 16 : alias_at + 32])
swapped_aliases[alias_at : alias_at + 16] = row1
swapped_aliases[alias_at + 16 : alias_at + 32] = row0
expect_status("alias row ordering", 251, lambda: c.decode(bytes(swapped_aliases)))
duplicate_alias = bytearray(encoded)
duplicate_alias[alias_at + 16 + 4 : alias_at + 16 + 8] = duplicate_alias[alias_at + 4 : alias_at + 8]
expect_status("requester-local alias duplicate", 251, lambda: c.decode(bytes(duplicate_alias)))
for bad in ("Dep", "_dep", "dep_", "de__p", "de-p"):
    altered = copy.deepcopy(basic_manifest); altered["aliases"][0]["alias"] = bad
    expect_status(f"alias spelling {bad!r}", 251, lambda m=altered: c.encode_manifest(m, basic_bundle))
cycle = copy.deepcopy(basic_manifest)
cycle["aliases"].append({"requester": key(3), "alias": "back", "target": key(1)})
expect_status("alias graph cycle", 251, lambda: c.encode_manifest(cycle, basic_bundle))
unreachable_bundle = bundle([("a/root.omg", b"a"), ("b/lib.omg", b"b")])
unreachable = manifest([package(1, [("a/root.omg", "")]), package(2, [("b/lib.omg", "lib")])], [])
expect_status("unreachable package", 251, lambda: c.encode_manifest(unreachable, unreachable_bundle))

# String-table order/count/extent, role intersections, paths, and unused rows.
rows, bundle_at = string_rows(encoded)
non_ascii = bytearray(encoded); non_ascii[rows["lib"][0]] = 0xFF
expect_status("non-ASCII string", 251, lambda: c.decode(bytes(non_ascii)))
duplicate_string = bytearray(encoded)
duplicate_string[rows["leaf"][0] : rows["leaf"][0] + 4] = b"main"
expect_status("string uniqueness/order", 251, lambda: c.decode(bytes(duplicate_string)))
expect_status("string count mismatch", 251, lambda: c.decode(replace_u32(encoded, 28, len(decoded.strings) - 1)))
bad_string_length = bytearray(encoded)
_, _, _, strings_at = layout(encoded)
struct.pack_into("<I", bad_string_length, strings_at, c.HEADER.unpack_from(encoded)[7])
expect_status("string payload truncation", 251, lambda: c.decode(bytes(bad_string_length)))
unused = bytearray(encoded)
lib_id = decoded.strings.index("lib"); other_id = decoded.strings.index("other")
struct.pack_into("<I", unused, source_at + 20 + 12, other_id)
expect_status("unused string", 251, lambda: c.decode(bytes(unused)))
empty_alias = replace_u32(encoded, alias_at + 4, decoded.strings.index(""))
expect_status("empty string in alias role", 251, lambda: c.decode(empty_alias))
owner_alias = replace_u32(encoded, alias_at + 4, decoded.strings.index("Owner"))
expect_status("multi-role string must satisfy alias role", 251, lambda: c.decode(owner_alias))
invalid_module = bytearray(encoded); invalid_module[rows["lib"][0]] = ord("1")
expect_status("invalid module component", 251, lambda: c.decode(bytes(invalid_module)))

for root_field, bad_value in (("owner", "A::B"), ("machine", "bad-name"), ("owner", "9Bad")):
    altered = copy.deepcopy(basic_manifest); altered["root"][root_field] = bad_value
    expect_status(f"root {root_field} identifier", 251, lambda m=altered: c.encode_manifest(m, basic_bundle))
expect_status("root package range", 251, lambda: c.decode(replace_u32(encoded, 44, 3)))
expect_status("root source range", 251, lambda: c.decode(replace_u32(encoded, 48, 3)))
expect_status("root source ownership", 251, lambda: c.decode(replace_u32(encoded, 48, 1)))
expect_status("root owner string range", 251, lambda: c.decode(replace_u32(encoded, 52, 99)))
expect_status("root machine string range", 251, lambda: c.decode(replace_u32(encoded, 56, 99)))

# Nested bundle identity, canonical labels, exact count, and exact EOF.
bad_nested_magic = bytearray(encoded); bad_nested_magic[bundle_at] ^= 1
expect_status("nested bundle magic", 251, lambda: c.decode(bytes(bad_nested_magic)))
bad_nested_label = bytearray(encoded); bad_nested_label[bundle_at + b.HEADER.size + b.ENTRY_HEADER.size] = ord("/")
expect_status("nested bundle label", 251, lambda: c.decode(bytes(bad_nested_label)))
bad_nested_count = replace_u32(encoded, bundle_at + 12, 2)
expect_status("nested bundle count/EOF", 251, lambda: c.decode(bad_nested_count))
nested_17 = bundle([(f"s{index:02d}.omg", b"") for index in range(17)])
nested_count_exhausted = bytearray(encoded[:bundle_at] + nested_17)
struct.pack_into("<I", nested_count_exhausted, 16, len(nested_count_exhausted))
struct.pack_into("<I", nested_count_exhausted, 20, len(nested_17))
expect_status("nested bundle source count adjacent", 252, lambda: c.decode(bytes(nested_count_exhausted)))
nested_trailing = bytearray(encoded + b"x")
struct.pack_into("<I", nested_trailing, 16, len(nested_trailing))
struct.pack_into("<I", nested_trailing, 20, len(basic_bundle) + 1)
expect_status("nested bundle trailing byte", 251, lambda: c.decode(bytes(nested_trailing)))

# Exact public resource boundaries and their independently realizable adjacent teeth.
labels64 = [(f"{index:02d}/" + chr(97 + index) + "x" * 60, b"") for index in range(16)]
assert all(len(label.encode("ascii")) == 64 for label, _ in labels64)
max_bundle = bundle([(label, b"x" * 131_072 if index < 2 else content) for index, (label, content) in enumerate(labels64)])
assert len(max_bundle) == c.MAX_BUNDLE_BYTES
max_sources = manifest([package(1, [(label, "") for label, _ in labels64])], [], root=(1, labels64[0][0], "Owner", "main"))
max_encoded = c.encode_manifest(max_sources, max_bundle)
assert len(c.decode(max_encoded).sources) == 16
assert sum(len(entry.content) for entry in c.decode(max_encoded).bundle_entries) == 262_144

label65 = "a/" + "x" * 63
expect_status(
    "source label 65",
    252,
    lambda: c.encode_manifest(manifest([package(1, [(label65, "")])], [], root=(1, label65, "Owner", "main")), bundle([(label65, b"")])),
)
content_over_bundle = bundle([("a/root.omg", b"x" * 131_073)])
expect_status("per-source content adjacent", 252, lambda: c.encode_manifest(manifest([package(1, [("a/root.omg", "")])], []), content_over_bundle))
aggregate_over_bundle = bundle([("a/root.omg", b"x" * 131_072), ("b.omg", b"x" * 131_072), ("c.omg", b"x")])
aggregate_over_manifest = manifest([package(1, [("a/root.omg", ""), ("b.omg", "b"), ("c.omg", "c")])], [])
expect_status("aggregate content adjacent", 252, lambda: c.encode_manifest(aggregate_over_manifest, aggregate_over_bundle))

packages16 = [package(index, [(f"p{index:02d}.omg", f"m{index:02d}")]) for index in range(1, 17)]
bundle16 = bundle([(f"p{index:02d}.omg", b"") for index in range(1, 17)])
chain = [(index, f"p{index + 1:02d}", index + 1) for index in range(1, 16)]
assert len(c.decode(c.encode_manifest(manifest(packages16, chain, root=(1, "p01.omg", "Owner", "main")), bundle16)).packages) == 16

aliases32 = [(1, f"a{index:02d}", index + 2) for index in range(15)]
aliases32 += [(2, f"b{index:02d}", index + 3) for index in range(14)]
aliases32 += [(3, f"c{index:02d}", index + 4) for index in range(3)]
assert len(aliases32) == 32
alias_max_manifest = manifest(packages16, aliases32, root=(1, "p01.omg", "Owner", "main"))
assert len(c.decode(c.encode_manifest(alias_max_manifest, bundle16)).aliases) == 32
alias_over = copy.deepcopy(alias_max_manifest)
alias_over["aliases"].append({"requester": key(4), "alias": "overflow", "target": key(16)})
expect_status("alias count adjacent", 252, lambda: c.encode_manifest(alias_over, bundle16))

modules = []
for index in range(16):
    second_length = 61 if index < 9 else 62
    first = chr(97 + index) + "x" * 63
    second = chr(97 + index) + "y" * (second_length - 1)
    modules.append(first + "::" + second)
assert sum(map(len, modules)) + len("Owner") + len("main") == 2_048
payload_manifest = manifest(
    [package(1, [(f"s{index:02d}.omg", modules[index]) for index in range(16)])],
    [],
    root=(1, "s00.omg", "Owner", "main"),
)
payload_bundle = bundle([(f"s{index:02d}.omg", b"") for index in range(16)])
payload_encoded = c.encode_manifest(payload_manifest, payload_bundle)
assert sum(len(item.encode("ascii")) for item in c.decode(payload_encoded).strings) == 2_048
payload_over = copy.deepcopy(payload_manifest)
payload_over["packages"][0]["sources"][0]["module"] += "z"
expect_status("string payload adjacent", 252, lambda: c.encode_manifest(payload_over, payload_bundle))

path8 = "::".join("a" + str(index) for index in range(8))
path_manifest = manifest([package(1, [("a/root.omg", path8)])], [])
c.decode(c.encode_manifest(path_manifest, bundle([("a/root.omg", b"")])))
path9 = path8 + "::a8"
path_over = manifest([package(1, [("a/root.omg", path9)])], [])
expect_status("path components adjacent", 252, lambda: c.encode_manifest(path_over, bundle([("a/root.omg", b"")])))
component64 = "a" * 64
c.decode(c.encode_manifest(manifest([package(1, [("a/root.omg", component64)])], []), bundle([("a/root.omg", b"")])))
component65 = "a" * 65
expect_status(
    "identifier bytes adjacent",
    252,
    lambda: c.encode_manifest(manifest([package(1, [("a/root.omg", component65)])], []), bundle([("a/root.omg", b"")])),
)

# With unused strings forbidden, v1 has at most 16+32+2 = 50 reference slots.
# Exercise that realizable maximum; the declared 65-row exhaustion tooth is above.
max_string_manifest = copy.deepcopy(alias_max_manifest)
max_string_manifest["root"]["owner"] = "RootOwner"
max_string_manifest["root"]["machine"] = "root_machine"
max_string_encoded = c.encode_manifest(max_string_manifest, bundle16)
assert len(c.decode(max_string_encoded).strings) == 50

print("omega-bootstrap compilation: canonical envelope and malformed/resource contract teeth pass")
PY
