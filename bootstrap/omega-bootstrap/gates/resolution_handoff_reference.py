#!/usr/bin/env python3
"""Independent fixtures and structural inspection for the OMGRSW1 producer.

This tool is untrusted gate plumbing.  It neither resolves the producer's input
for it nor grants receipt/digest authority.
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from dataclasses import dataclass
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
sys.path.insert(0, str(COMPILER))

import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


DEP_KEY = "11" * 32
ROOT_KEY = "22" * 32
NO_ID = 0xFFFFFFFF
HEADER = struct.Struct("<8sHHHH14I")
TABLES = (
    ("units", 36), ("imports", 48), ("bindings", 28),
    ("declarations", 28), ("types", 24), ("records", 24),
    ("fields", 24), ("machines", 40), ("machine_parameters", 24),
    ("blocks", 40), ("block_parameters", 24),
)


class ResolutionError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ResolutionError(message)


@dataclass(frozen=True)
class Witness:
    raw: bytes
    counts: tuple[int, ...]
    selected: int
    offsets: dict[str, int]

    def row(self, table: str, index: int) -> bytes:
        stride = dict(TABLES)[table]
        count = self.counts[[name for name, _ in TABLES].index(table)]
        require(0 <= index < count, f"{table} row out of range")
        start = self.offsets[table] + index * stride
        return self.raw[start:start + stride]


def decode_witness(raw: bytes) -> Witness:
    require(len(raw) >= HEADER.size, "truncated OMGRSW1 header")
    values = HEADER.unpack_from(raw)
    require(values[:5] == (b"OMGRSW1\0", 1, 0, 0, 72), "OMGRSW1 fixed header")
    length = values[5]
    counts = tuple(values[6:17])
    selected, reserved = values[17:19]
    require(reserved == 0 and length == len(raw), "OMGRSW1 length/reserved")
    ceilings = (16, 64, 4096, 256, 2048, 128, 4096, 128, 2048, 2048, 4096)
    require(all(count <= ceiling for count, ceiling in zip(counts, ceilings)), "OMGRSW1 count ceiling")
    offsets: dict[str, int] = {}
    pos = HEADER.size
    for (name, stride), count in zip(TABLES, counts):
        offsets[name] = pos
        require(count <= (len(raw) - pos) // stride, f"{name} extent")
        pos += count * stride
    require(pos == len(raw), "OMGRSW1 exact EOF")
    witness = Witness(raw, counts, selected, offsets)
    for (name, _), count in zip(TABLES, counts):
        for index in range(count):
            require(struct.unpack_from("<I", witness.row(name, index))[0] == index, f"{name} dense ID")
    return witness


def source_contents(envelope: bytes) -> tuple[bytes, ...]:
    decoded = compilation.decode(envelope)
    entries = decoded.bundle_entries
    return tuple(entries[row.bundle_entry_id].content for row in decoded.sources)


def span(source: bytes, start: int, length: int) -> bytes:
    require(start <= len(source) and length <= len(source) - start, "source span")
    return source[start:start + length]


def check_canonical(envelope_path: Path, witness_path: Path) -> None:
    envelope = envelope_path.read_bytes()
    witness = decode_witness(witness_path.read_bytes())
    require(witness.counts == (2, 1, 2, 3, 5, 2, 3, 1, 0, 1, 0), "canonical table counts")
    require(witness.selected == 0, "canonical selected machine")
    sources = source_contents(envelope)

    unit0 = struct.unpack("<9I", witness.row("units", 0))
    unit1 = struct.unpack("<9I", witness.row("units", 1))
    require(unit0[1:3] == (0, 3) and unit1[1:3] == (1, 1), "canonical unit identity")
    require(span(sources[0], unit0[3], unit0[4]) == b"model", "dependency authored module")
    require(span(sources[1], unit1[3], unit1[4]) == b"app", "root authored module")
    require(unit0[5:] == (0, 0, 0, 1) and unit1[5:] == (0, 1, 1, 2), "unit partitions")

    imported = witness.row("imports", 0)
    iid, source_id, ordinal, path_start, path_len = struct.unpack_from("<5I", imported)
    origin, target_kind = imported[20], imported[21]
    alias_id, target_package, target_module, declaration, local_start, local_len = struct.unpack_from("<6I", imported, 24)
    require((iid, source_id, ordinal, origin, target_kind) == (0, 1, 0, 1, 1), "canonical import identity")
    require((alias_id, target_package, target_module, declaration) == (0, 0, 3, 0), "canonical import target")
    require(span(sources[1], path_start, path_len) == b"dep::model::Pair", "canonical import path")
    require(span(sources[1], local_start, local_len) == b"Pair", "canonical import local")

    bindings = []
    for index in range(2):
        row = witness.row("bindings", index)
        bid, source_id = struct.unpack_from("<2I", row)
        role, kind = row[8], row[9]
        start, length, declaration, import_id = struct.unpack_from("<4I", row, 12)
        bindings.append((bid, source_id, role, kind, span(sources[source_id], start, length), declaration, import_id))
    require(bindings == [(0, 1, 1, 1, b"Pair", 0, 0), (1, 1, 2, 1, b"Probe", 1, NO_ID)], "canonical static bindings")

    declarations = []
    for index in range(3):
        row = witness.row("declarations", index)
        did = struct.unpack_from("<I", row)[0]
        kind, visibility = row[4], row[5]
        source_id, ordinal, start, length, kind_id = struct.unpack_from("<5I", row, 8)
        declarations.append((did, kind, visibility, source_id, ordinal, span(sources[source_id], start, length), kind_id))
    require(declarations == [
        (0, 1, 1, 0, 0, b"Pair", 0),
        (1, 1, 0, 1, 0, b"Probe", 1),
        (2, 2, 0, 1, 1, b"run", 0),
    ], "canonical declarations")

    types = [struct.unpack("<IBBHIIII", witness.row("types", i)) for i in range(5)]
    require(types == [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 4, 0, 0, 1, 0, 0, 0),
        (2, 3, 0, 0, 0, 0, 0, 1),
        (3, 2, 0, 0, 0, 0, 0, 0x7FFFFFFF),
        (4, 1, 0, 0, 0, 0, 0, 255),
    ], "canonical type order")

    records = []
    for index in range(2):
        row = witness.row("records", index)
        records.append((*struct.unpack_from("<5I", row), row[20]))
        require(row[21:24] == b"\0\0\0", "canonical record reserved")
    require(records == [(0, 0, 0, 0, 2, 1), (1, 1, 1, 2, 1, 0)], "canonical records")

    fields = []
    for index in range(3):
        fid, owner, ordinal, type_id, start, length = struct.unpack("<6I", witness.row("fields", index))
        fields.append((fid, owner, ordinal, type_id, span(sources[0 if owner == 0 else 1], start, length)))
    require(fields == [
        (0, 0, 0, 4, b"first"),
        (1, 0, 1, 4, b"second"),
        (2, 1, 0, 0, b"pair"),
    ], "canonical fields")

    machine = witness.row("machines", 0)
    require(struct.unpack_from("<3I", machine) == (0, 2, 1), "canonical machine identity")
    require(machine[12:16] == b"\x02\0\0\0", "canonical machine receiver/reserved")
    require(struct.unpack_from("<6I", machine, 16) == (4, 0, 0, 0, 1, 0), "canonical machine partitions")

    block = witness.row("blocks", 0)
    require(struct.unpack_from("<3I", block) == (0, 0, 0), "canonical block identity")
    require(block[12:16] == b"\x02\0\0\0", "canonical block receiver/reserved")
    require(struct.unpack_from("<6I", block, 16) == (110, 179, NO_ID, 0, 0, 0), "canonical body custody/partitions")
    require(sources[1][110:179].startswith(b"self.pair.first") and sources[1][110:179].rstrip().endswith(b"self.pair.first"), "canonical body span")

    require(imported[22:24] == b"\0\0", "canonical import reserved")
    require(all(witness.row("bindings", i)[10:12] == b"\0\0" for i in range(2)), "canonical binding reserved")
    require(all(witness.row("declarations", i)[6:8] == b"\0\0" for i in range(3)), "canonical declaration reserved")
    require(all(witness.row("types", i)[6:8] == b"\0\0" for i in range(5)), "canonical type reserved")
    require(witness.counts[8] == 0 and witness.counts[10] == 0, "canonical empty parameter tables")
    print("resolution handoff canonical witness: exact modules/imports/bindings/root passed")


def make_manifest(packages: list[dict[str, object]], aliases: list[dict[str, str]], root_source: str, root_owner: str = "Probe") -> dict[str, object]:
    return {
        "target": "linux_x86_64", "packages": packages, "aliases": aliases,
        "root": {"package": ROOT_KEY if any(p["key"] == ROOT_KEY for p in packages) else DEP_KEY,
                 "source": root_source, "owner": root_owner, "machine": "run"},
    }


def pkg(key: str, specs: list[tuple[str, str]]) -> dict[str, object]:
    return {"key": key, "sources": [{"label": label, "module": module} for label, module in specs]}


def encode(entries: list[tuple[str, str]], manifest: dict[str, object]) -> bytes:
    packed = bundle.encode([bundle.Entry(label, text.encode("ascii")) for label, text in entries])
    return compilation.encode_manifest(manifest, packed)


def one_source(text: str, *, label: str = "main.omg", module: str = "app", owner: str = "Probe") -> bytes:
    return encode([(label, text)], make_manifest([pkg(DEP_KEY, [(label, module)])], [], label, owner))


def resource_cases() -> list[tuple[str, int, bytes]]:
    cases: list[tuple[str, int, bytes]] = []
    add = lambda name, status, data: cases.append((name, status, data))

    qualified_entries = [
        ("a-model.omg", "module model; pub data Pair [copy] { x: u8; }\n"),
        ("b-app.omg", "module app; data Probe { p: model::Pair; } machine Probe::run(&self) -> u32 { 0 }\n"),
    ]
    qualified_manifest = make_manifest([pkg(DEP_KEY, [(x, "model" if x.startswith("a") else "app") for x, _ in qualified_entries])], [], "b-app.omg")
    add("qualified-same-package", 0, encode(qualified_entries, qualified_manifest))
    missing = [(label, text.replace("model::Pair", "missing::Pair")) for label, text in qualified_entries]
    add("qualified-missing-module", 251, encode(missing, qualified_manifest))

    ambiguous_entries = [
        ("dep/lib.omg", "module lib; pub data Other {}\n"),
        ("root/model.omg", "module model; pub data Pair [copy] { x: u8; }\n"),
        ("root/main.omg", "module app; data Probe { p: model::Pair; } machine Probe::run(&self) -> u32 { 0 }\n"),
    ]
    ambiguous_manifest = make_manifest(
        [pkg(DEP_KEY, [("dep/lib.omg", "lib")]), pkg(ROOT_KEY, [("root/main.omg", "app"), ("root/model.omg", "model")])],
        [{"requester": ROOT_KEY, "alias": "model", "target": DEP_KEY}], "root/main.omg")
    add("qualified-alias-module-ambiguity", 251, encode(ambiguous_entries, ambiguous_manifest))

    private_same_module_entries = [
        ("dep/app.omg", "module app; data Pair [copy] { x: u8; }\n"),
        ("root/main.omg", "module app; use dep::app::Pair; data Probe { p: Pair; } machine Probe::run(&self) -> u32 { 0 }\n"),
    ]
    private_same_module_manifest = make_manifest(
        [pkg(DEP_KEY, [("dep/app.omg", "app")]), pkg(ROOT_KEY, [("root/main.omg", "app")])],
        [{"requester": ROOT_KEY, "alias": "dep", "target": DEP_KEY}], "root/main.omg")
    add("private-cross-package-same-module-spelling", 251, encode(private_same_module_entries, private_same_module_manifest))

    arrays = "data Probe { a: [u8; 2]; middle: u32 [0..=7]; b: [u8; 3]; } machine Probe::run(&self) -> u32 { 0 }\n"
    add("array-order", 0, one_source(arrays, module=""))
    nested_arrays = "module app; data Probe { a: [[u8; 2]; 2]; b: [u8; 3]; } machine Probe::run(&self) -> u32 { 0 }\n"
    add("nested-array-resolution", 0, one_source(nested_arrays))

    spaced_entries = [
        ("dep/model.omg", "module model :: nested; pub data Pair [copy] { x: u8; }\n"),
        ("root/main.omg", "module app :: main; use dep :: model :: nested :: Pair; data Probe { imported: Pair; local: model :: nested :: Local; } machine Probe::run(&self) -> u32 { 0 }\n"),
    ]
    spaced_manifest = make_manifest(
        [pkg(DEP_KEY, [("dep/model.omg", "model::nested")]), pkg(ROOT_KEY, [("root/local.omg", "model::nested"), ("root/main.omg", "app::main")])],
        [{"requester": ROOT_KEY, "alias": "dep", "target": DEP_KEY}], "root/main.omg")
    # The same-package qualified spelling is supplied by the root package's
    # second source; keep its declaration distinct from the imported Pair.
    spaced_entries.append(("root/local.omg", "module model :: nested; pub data Local {}\n"))
    add("path-separator-trivia", 0, encode(spaced_entries, spaced_manifest))

    quoted = "module app; data Probe {} machine Probe::run(&self) -> u32 { \"}\"; 0 }\n"
    add("brace-in-string-rejected", 251, one_source(quoted))

    id64 = "f" * 64
    add("identifier-64", 0, one_source(f"module app; data Probe {{ {id64}: u8; }} machine Probe::run(&self) -> u32 {{ 0 }}\n"))
    add("identifier-65", 252, one_source(f"module app; data Probe {{ {'f' * 65}: u8; }} machine Probe::run(&self) -> u32 {{ 0 }}\n"))

    def import_case(count: int) -> bytes:
        dep = "module model;\n" + "".join(f"pub data D{i:02d} {{}}\n" for i in range(count))
        root = "module app;\n" + "".join(f"use dep::model::D{i:02d};\n" for i in range(count)) + "data Probe {} machine Probe::run(&self) -> u32 { 0 }\n"
        entries = [("dep/model.omg", dep), ("root/main.omg", root)]
        manifest = make_manifest(
            [pkg(DEP_KEY, [("dep/model.omg", "model")]), pkg(ROOT_KEY, [("root/main.omg", "app")])],
            [{"requester": ROOT_KEY, "alias": "dep", "target": DEP_KEY}], "root/main.omg")
        return encode(entries, manifest)
    add("imports-64", 0, import_case(64)); add("imports-65", 252, import_case(65))

    def declaration_machine_case(machines: int, records: int = 128) -> bytes:
        text = "module app;\n" + "".join(f"data R{i:03d} {{}}\n" for i in range(records))
        text += "machine R000::run(&self) -> u32 { 0 }\n"
        text += "".join(f"machine R000::m{i:03d}(&self) {{}}\n" for i in range(1, machines))
        return one_source(text, owner="R000")
    add("records-128-machines-128-declarations-256", 0, declaration_machine_case(128))
    add("machines-129-declarations-257", 252, declaration_machine_case(129))
    records129 = "module app;\n" + "".join(f"data R{i:03d} {{}}\n" for i in range(129)) + "machine R000::run(&self) -> u32 { 0 }\n"
    add("records-129", 252, one_source(records129, owner="R000"))

    def fields_case(extra: int) -> bytes:
        chunks = ["module app;\n"]
        for record in range(64):
            chunks.append(f"data R{record:02d} {{ " + "".join(f"f{i:02d}: u8;" for i in range(64)) + " }\n")
        if extra:
            chunks.append("data Extra { one: u8; }\n")
        chunks.append("machine R00::run(&self) -> u32 { 0 }\n")
        return one_source("".join(chunks), owner="R00")
    add("fields-4096", 0, fields_case(0)); add("fields-4097", 252, fields_case(1))

    def binding_case(over: bool) -> bytes:
        chunks = ["module app; data Base {}\n"]
        remaining = 4096 if over else 4095
        record = 0
        while remaining:
            count = min(64, remaining); remaining -= count
            chunks.append(f"data B{record:02d} {{ " + "".join(f"f{i:02d}: Base;" for i in range(count)) + " }\n")
            record += 1
        chunks.append("machine Base::run(&self) -> u32 { 0 }\n")
        return one_source("".join(chunks), owner="Base")
    add("bindings-4096", 0, binding_case(False)); add("bindings-4097", 252, binding_case(True))

    def type_case(last_fields: int) -> bytes:
        chunks = ["module app;\n"]; value = 0
        for record in range(31):
            fields = []
            for field in range(64):
                fields.append(f"f{field:02d}: u32 [0..={value}];"); value += 1
            chunks.append(f"data T{record:02d} {{ " + "".join(fields) + " }\n")
        fields = []
        for field in range(last_fields):
            fields.append(f"z{field:02d}: u32 [0..={value}];"); value += 1
        chunks.append("data T31 { " + "".join(fields) + " }\n")
        chunks.append("machine T00::run(&self) -> u32 { 0 }\n")
        return one_source("".join(chunks), owner="T00")
    add("types-2048", 0, type_case(30)); add("types-2049", 252, type_case(31))

    def block_case(extra_block: bool = False, extra_param: bool = False) -> bytes:
        entries: list[tuple[str, str]] = []
        specs: list[tuple[str, str]] = []
        state_index = 0
        for machine in range(16):
            lines = ["module app;\n"]
            if machine == 0:
                lines.append("data Probe {}\n")
            name = "run" if machine == 0 else f"m{machine:02d}"
            result = " -> u32" if machine == 0 else ""
            lines.append(f"machine Probe::{name}(&self){result} {{\n")
            for state in range(127):
                count = 3 if state_index < 32 else 2
                if extra_param and state_index == 32:
                    count = 3
                params = "".join(f", p{i}: u8" for i in range(count))
                lines.append(f"state s{state:03d}(&self{params}) {{}}\n")
                state_index += 1
            lines.append("}\n")
            if extra_block and machine == 0:
                lines.append("machine Probe::overflow(&self) {}\n")
            label = f"s{machine:02d}.omg"; entries.append((label, "".join(lines))); specs.append((label, "app"))
        return encode(entries, make_manifest([pkg(DEP_KEY, specs)], [], "s00.omg"))
    add("units-16-blocks-2048-block-parameters-4096", 0, block_case())
    add("blocks-2049", 252, block_case(extra_block=True))
    add("block-parameters-4097", 252, block_case(extra_param=True))
    return cases


def build_controls(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    index = []
    for name, status, envelope in resource_cases():
        path = output / f"{name}.omgc"
        path.write_bytes(envelope)
        index.append({"name": name, "status": status, "bytes": len(envelope)})
    (output / "index.json").write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
    (output / "array-order.omg").write_text(
        "data Probe { a: [u8; 2]; middle: u32 [0..=7]; b: [u8; 3]; } machine Probe::run(&self) -> u32 { 0 }\n",
        encoding="ascii",
    )


def check_control(name: str, witness_path: Path) -> None:
    witness = decode_witness(witness_path.read_bytes())
    if name == "nested-array-resolution":
        types = [struct.unpack("<IBBHIIII", witness.row("types", i)) for i in range(witness.counts[4])]
        require([(row[1], row[4], row[5]) for row in types] == [
            (4, 0, 0), (3, 0, 0), (2, 0, 0), (1, 0, 0),
            (5, 3, 2), (5, 4, 2), (5, 3, 3),
        ], "nested array dependency/order")
    if name == "qualified-same-package":
        bindings = [witness.row("bindings", i) for i in range(witness.counts[2])]
        require(any(row[8] == 1 and struct.unpack_from("<I", row, 24)[0] == NO_ID for row in bindings), "qualified binding has no import")
    print(f"resolution handoff control valid: {name}")


def check_type_parity(witness_path: Path, ckir_path: Path) -> None:
    witness = decode_witness(witness_path.read_bytes())
    ckir = ckir_path.read_bytes()
    require(len(ckir) >= 72 and ckir[:8] == b"OMGCKIR\0", "CKIR1 header")
    type_count = struct.unpack_from("<I", ckir, 24)[0]
    require(type_count == witness.counts[4], "type count parity")
    ckir_types = ckir[72:72 + type_count * 24]
    witness_types = witness.raw[witness.offsets["types"]:witness.offsets["types"] + type_count * 24]
    require(ckir_types == witness_types, "type rows differ from frozen producer")
    print("resolution handoff type rows: frozen producer parity passed")


def main() -> None:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    canonical = commands.add_parser("check-canonical")
    canonical.add_argument("envelope", type=Path); canonical.add_argument("witness", type=Path)
    build = commands.add_parser("build-controls"); build.add_argument("output", type=Path)
    control = commands.add_parser("check-control"); control.add_argument("name"); control.add_argument("witness", type=Path)
    parity = commands.add_parser("check-type-parity"); parity.add_argument("witness", type=Path); parity.add_argument("ckir", type=Path)
    args = parser.parse_args()
    if args.command == "check-canonical":
        check_canonical(args.envelope, args.witness)
    elif args.command == "build-controls":
        build_controls(args.output)
    elif args.command == "check-control":
        check_control(args.name, args.witness)
    else:
        check_type_parity(args.witness, args.ckir)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ResolutionError, compilation.CompilationError, bundle.BundleError, struct.error) as error:
        raise SystemExit(f"resolution handoff reference: {error}")
