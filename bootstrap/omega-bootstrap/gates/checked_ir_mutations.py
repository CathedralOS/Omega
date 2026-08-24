#!/usr/bin/env python3
"""Generate schema-aware CKIR1 negative controls from an all-operation fixture.

This is deliberately independent of ``checked_ir_reference.py``.  It decodes only
the published CKIR1 wire schema needed to locate mutation sites; it is an
untrusted corpus generator, never a validator or an authority over acceptance.

Usage:

    checked_ir_mutations.py INPUT.ckir OUTPUT_DIR [MANIFEST.tsv]

The manifest defaults to ``OUTPUT_DIR/manifest.tsv``.  Its paths are relative to
OUTPUT_DIR so a caller can relocate the generated corpus as one directory.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import struct


NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH14I")


@dataclass(frozen=True)
class TableSpec:
    row: struct.Struct
    count_index: int


TABLES = {
    "types": TableSpec(struct.Struct("<IBBHIIII"), 0),
    "records": TableSpec(struct.Struct("<IIIIB3x"), 1),
    "fields": TableSpec(struct.Struct("<IIII"), 2),
    "machines": TableSpec(struct.Struct("<IIBBHIIIIII"), 3),
    "machine_params": TableSpec(struct.Struct("<IIIII"), 4),
    "blocks": TableSpec(struct.Struct("<IIBBHIIIII"), 5),
    "block_params": TableSpec(struct.Struct("<IIIII"), 6),
    "operations": TableSpec(struct.Struct("<IIIBBHIIIIII"), 7),
    "operands": TableSpec(struct.Struct("<I"), 8),
    "terminators": TableSpec(struct.Struct("<IIIBBHIIIIIII"), 9),
}

# Header count name, byte offset, published ceiling.  Terminators have a
# relation (one per block), not an independent published table ceiling.
HEADER_COUNTS = (
    ("type", 24, 8_192),
    ("record", 28, 128),
    ("field", 32, 8_192),
    ("machine", 36, 128),
    ("machine-param", 40, 896),
    ("block", 44, 2_048),
    ("block-param", 48, 4_096),
    ("operation", 52, 32_768),
    ("operand", 56, 94_208),
    ("terminator", 60, None),
    ("value", 64, 36_864),
    ("place", 68, 32_768),
)

ID_TABLES = (
    "types", "records", "fields", "machines", "machine_params", "blocks",
    "block_params", "operations", "terminators",
)

# These shapes cannot be obtained from the source-custody fixture by an honest
# local relation mutation.  The resource corpus must provide the named controls
# so acceptance and the isolated shared-Copy rejection are both proved.  Keeping
# this inventory beside the negative generator prevents its all-operation input
# from being mistaken for complete terminator/structural-value coverage.
EXTERNAL_CONTROLS = (
    (
        "structural-jump-control.ckir", "0-empty",
        "canonical library module containing valid Jump and ReturnUnit rows, "
        "a Jump that passes a copyable structural machine parameter to an "
        "exact-type structural block parameter, and Copy immediate 0 equal to "
        "1 from that structural value into an exact-type mutable "
        "receiver-derived place",
    ),
    (
        "structural-shared-copy.ckir", "251-empty",
        "the structural-jump control with only the Copy destination made "
        "shared while its block and entry signatures remain canonical",
    ),
)

# Equality, rather than a prefix or minimum-count check, makes additions and
# accidental removals visible during corpus generation.
EXPECTED_MUTATION_CLASSES: frozenset[str] = frozenset({
    "block.flags",
    "block.reserved",
    "count.block-capacity",
    "count.block-param-capacity",
    "count.combined-parameter-capacity",
    "count.field-capacity",
    "count.machine-capacity",
    "count.machine-param-capacity",
    "count.non-d0-word",
    "count.operand-capacity",
    "count.operation-capacity",
    "count.place-capacity",
    "count.record-capacity",
    "count.terminator-block-relation",
    "count.type-capacity",
    "count.value-capacity",
    "edge.arity",
    "envelope.non-d0-word",
    "envelope.trailing",
    "envelope.truncated",
    "header.entry-candidate",
    "header.entry-flag-bit",
    "header.entry-flag-relation",
    "header.flags",
    "header.magic",
    "header.schema-major",
    "header.schema-minor",
    "header.target",
    "header.total-length",
    "id.block-parameter-value",
    "id.block-terminator",
    "id.block_params.dense",
    "id.blocks.dense",
    "id.fields.dense",
    "id.machine-entry-block",
    "id.machine-parameter-value",
    "id.machine_params.dense",
    "id.machines.dense",
    "id.operations.dense",
    "id.records.dense",
    "id.terminator-value",
    "id.terminators.dense",
    "id.types.dense",
    "layout.recursive-by-value",
    "machine.flags",
    "machine.reserved",
    "operand.cross-block",
    "operand.cross-machine",
    "operand.edge-visibility",
    "operand.place-cross-block",
    "operand.place-cross-machine",
    "operand.place-reference",
    "operand.place-use-before-definition",
    "operand.reference",
    "operand.type",
    "operand.use-before-definition",
    "operation.arity",
    "operation.constant-range",
    "operation.copy-mode",
    "operation.field-immediate",
    "operation.flags",
    "operation.immediate",
    "operation.immediate-zero",
    "operation.index-place-base",
    "operation.noncopyable-copy",
    "operation.opcode",
    "operation.reconstructed-place-count",
    "operation.reconstructed-value-count",
    "operation.result-id",
    "operation.result-kind",
    "operation.result-type",
    "operation.shared-receiver-store",
    "ordinal.block-parameter",
    "ordinal.field",
    "ordinal.machine-parameter",
    "owner.block",
    "owner.block-parameter",
    "owner.field",
    "owner.machine",
    "owner.machine-parameter",
    "owner.operation-block",
    "owner.operation-machine",
    "owner.terminator-block",
    "owner.terminator-machine",
    "receiver-access.block",
    "receiver-access.block-exceeds-machine",
    "receiver-access.entry-block",
    "receiver-access.machine",
    "record.flags",
    "record.reserved",
    "result.less-bool",
    "result.add-carrier",
    "result.const-scalar-type",
    "result.field-place-type",
    "result.index-place-type",
    "result.load-type",
    "result.no-result-id",
    "result.no-result-type",
    "result.self-place-type",
    "span.block-operations",
    "span.block-parameter-count",
    "span.block-parameters",
    "span.edge-operands",
    "span.edge-target1-operands",
    "span.machine-block-count",
    "span.machine-blocks",
    "span.machine-parameter-count",
    "span.machine-parameters",
    "span.machine-parameters-zero",
    "span.operation-operands",
    "span.record-field-count",
    "span.record-fields",
    "status.252-not-overwritten-by-251",
    "target.cross-machine",
    "target.entry-block",
    "target.reference",
    "target.target1-cross-machine",
    "target.target1-entry-block",
    "terminator.branch-shape",
    "terminator.flags",
    "terminator.jump-shape",
    "terminator.kind",
    "terminator.reserved",
    "terminator.return-target-shape",
    "terminator.return-unit-shape",
    "terminator.return-value-shape",
    "type.block-parameter",
    "type.branch-condition",
    "type.copy-source",
    "type.edge-argument",
    "type.field",
    "type.flags",
    "type.forbidden-trapping-flag",
    "type.interning",
    "type.machine-parameter",
    "type.machine-result",
    "type.nominal-record-relation",
    "type.payload",
    "type.range",
    "type.reserved",
    "type.return-carrier",
    "type.tag",
})


class FixtureError(ValueError):
    pass


@dataclass(frozen=True)
class ValueInfo:
    type_id: int
    machine: int
    block: int | None
    definition: int


@dataclass(frozen=True)
class PlaceInfo:
    type_id: int
    machine: int
    block: int
    definition: int


class Fixture:
    def __init__(self, contents: bytes):
        if len(contents) < HEADER.size:
            raise FixtureError("fixture has a truncated CKIR1 header")
        self.contents = contents
        self.header = HEADER.unpack_from(contents)
        if self.header[0] != b"OMGCKIR\0" or self.header[1:4] != (1, 0, 1):
            raise FixtureError("fixture is not canonical CKIR1 target 1")
        self.entry = self.header[5]
        self.total = self.header[6]
        self.counts = tuple(self.header[7:])
        if self.total != len(contents):
            raise FixtureError("fixture total length does not match its bytes")

        self.offsets: dict[str, int] = {}
        self.rows: dict[str, list[tuple[int, ...]]] = {}
        cursor = HEADER.size
        for name, spec in TABLES.items():
            count = self.counts[spec.count_index]
            end = cursor + count * spec.row.size
            if end > len(contents):
                raise FixtureError(f"fixture truncates {name}")
            self.offsets[name] = cursor
            self.rows[name] = [
                spec.row.unpack_from(contents, cursor + index * spec.row.size)
                for index in range(count)
            ]
            cursor = end
        if cursor != len(contents):
            raise FixtureError("fixture has a noncanonical table extent")
        if self.entry == NO_ID or self.entry >= len(self.rows["machines"]):
            raise FixtureError("fixture has no selected conformance entry")

        self.values: dict[int, ValueInfo] = {}
        self.places: dict[int, PlaceInfo] = {}
        for row in self.rows["machine_params"]:
            row_id, owner, _, type_id, value_id = row
            if row_id != value_id:
                raise FixtureError("machine parameter value ID is not canonical")
            self.values[value_id] = ValueInfo(type_id, owner, None, -1)
        blocks = self.rows["blocks"]
        for row in self.rows["block_params"]:
            row_id, block, _, type_id, value_id = row
            if value_id != len(self.rows["machine_params"]) + row_id or block >= len(blocks):
                raise FixtureError("block parameter value ID is not canonical")
            self.values[value_id] = ValueInfo(type_id, blocks[block][1], block, -1)
        for row in self.rows["operations"]:
            op_id, machine, block, _, result_kind, _, result_id, result_type, *_ = row
            if result_kind == 1:
                self.values[result_id] = ValueInfo(result_type, machine, block, op_id)
            elif result_kind == 2:
                self.places[result_id] = PlaceInfo(result_type, machine, block, op_id)

    def row_offset(self, table: str, index: int) -> int:
        return self.offsets[table] + index * TABLES[table].row.size

    def reencode(self, replacements: dict[str, list[tuple[int, ...]]]) -> bytes:
        """Re-encode a mechanically adjusted corpus case from published rows."""
        tables = {
            name: replacements.get(name, list(self.rows[name]))
            for name in TABLES
        }
        payload = b"".join(
            TABLES[name].row.pack(*row)
            for name in TABLES
            for row in tables[name]
        )
        counts = tuple(len(tables[name]) for name in TABLES) + self.counts[10:]
        total = HEADER.size + len(payload)
        return HEADER.pack(
            b"OMGCKIR\0", 1, 0, 1, 1, self.entry, total, *counts,
        ) + payload

    def operands_for(self, operation: tuple[int, ...]) -> list[int]:
        return [row[0] for row in self.rows["operands"][operation[8]:operation[8] + operation[9]]]

    def operand_kinds(self, operation: tuple[int, ...]) -> tuple[str, ...]:
        opcode, copy_mode = operation[3], operation[10]
        return {
            1: (), 2: (), 3: ("place",), 4: ("place", "value"),
            5: ("place",), 6: ("place", "value"),
            7: ("place", "value" if copy_mode == 1 else "place"),
            8: ("value", "value"), 9: ("value", "value"),
        }.get(opcode, ())

    def visible_value(self, info: ValueInfo, operation: tuple[int, ...]) -> bool:
        op_id, machine, block = operation[0], operation[1], operation[2]
        return info.machine == machine and (
            info.block is None or info.block == block and info.definition < op_id
        )

    def visible_place(self, info: PlaceInfo, operation: tuple[int, ...]) -> bool:
        return (
            info.machine == operation[1]
            and info.block == operation[2]
            and info.definition < operation[0]
        )


class Corpus:
    def __init__(self, fixture: Fixture, output: Path, manifest: Path):
        self.fixture = fixture
        self.output = output
        self.manifest = manifest
        self.rows: list[tuple[str, int, str, int]] = []
        self.names: set[str] = set()

    def add(self, name: str, status: int, mutation_class: str, representative: int,
            payload: bytes | bytearray) -> None:
        if name in self.names:
            raise FixtureError(f"duplicate mutation name: {name}")
        if bytes(payload) == self.fixture.contents:
            raise FixtureError(f"mutation did not change the fixture: {name}")
        filename = f"{name}-{status}.ckir"
        (self.output / filename).write_bytes(payload)
        self.rows.append((filename, status, mutation_class, representative))
        self.names.add(name)

    def changed(self) -> bytearray:
        return bytearray(self.fixture.contents)

    def u8(self, name: str, status: int, mutation_class: str, representative: int,
           offset: int, value: int) -> None:
        payload = self.changed()
        struct.pack_into("<B", payload, offset, value)
        self.add(name, status, mutation_class, representative, payload)

    def u16(self, name: str, status: int, mutation_class: str, representative: int,
            offset: int, value: int) -> None:
        payload = self.changed()
        struct.pack_into("<H", payload, offset, value)
        self.add(name, status, mutation_class, representative, payload)

    def u32(self, name: str, status: int, mutation_class: str, representative: int,
            offset: int, value: int) -> None:
        payload = self.changed()
        struct.pack_into("<I", payload, offset, value & 0xFFFF_FFFF)
        self.add(name, status, mutation_class, representative, payload)

    def finish(self) -> None:
        classes = {row[2] for row in self.rows}
        if len(classes) != len(self.rows):
            raise FixtureError("mutation classes must be unique")
        if EXPECTED_MUTATION_CLASSES and classes != EXPECTED_MUTATION_CLASSES:
            missing = sorted(EXPECTED_MUTATION_CLASSES - classes)
            extra = sorted(classes - EXPECTED_MUTATION_CLASSES)
            raise FixtureError(f"mutation class inventory mismatch: missing={missing}, extra={extra}")
        lines = ["path\texpected_status\tclass\tself_representative"]
        lines.extend("\t".join(map(str, row)) for row in self.rows)
        self.manifest.write_text("\n".join(lines) + "\n", encoding="utf-8")
        controls = ["required_name\texpected_observation\texact_required_shape"]
        controls.extend("\t".join(row) for row in EXTERNAL_CONTROLS)
        (self.output / "required-external-controls.tsv").write_text(
            "\n".join(controls) + "\n", encoding="utf-8",
        )


def require_row(fixture: Fixture, table: str) -> tuple[int, tuple[int, ...]]:
    if not fixture.rows[table]:
        raise FixtureError(f"all-op fixture has no {table} row")
    return 0, fixture.rows[table][0]


def find_row(fixture: Fixture, table: str, predicate, description: str):
    for index, row in enumerate(fixture.rows[table]):
        if predicate(row):
            return index, row
    raise FixtureError(f"all-op fixture lacks {description}")


def generate_header(corpus: Corpus) -> None:
    fixture = corpus.fixture
    magic = corpus.changed()
    magic[0] ^= 1
    corpus.add("magic", 251, "header.magic", 1, magic)
    corpus.u16("schema-major", 251, "header.schema-major", 1, 8, 2)
    corpus.u16("schema-minor", 251, "header.schema-minor", 0, 10, 1)
    corpus.u16("target", 251, "header.target", 1, 12, 2)
    corpus.u16("flags-reserved", 251, "header.flags", 1, 14, 2)
    corpus.u16("entry-flag-bit", 251, "header.entry-flag-bit", 1, 14, 0)
    corpus.u32("entry-flag-relation", 251, "header.entry-flag-relation", 1, 16, NO_ID)

    alternate = next((row[0] for row in fixture.rows["machines"] if row[0] != fixture.entry), None)
    corpus.u32(
        "entry-candidate-relation", 251, "header.entry-candidate", 0, 16,
        alternate if alternate is not None else len(fixture.rows["machines"]),
    )
    corpus.u32("encoded-length", 251, "header.total-length", 1, 20, fixture.total + 1)
    corpus.u32("encoded-length-high-bit", 251, "envelope.non-d0-word", 1,
               20, 0x8000_0000)
    corpus.u32("count-high-bit", 251, "count.non-d0-word", 0, 24, 0x8000_0000)
    corpus.add("truncated", 251, "envelope.truncated", 1, fixture.contents[:-1])
    corpus.add("trailing", 251, "envelope.trailing", 0, fixture.contents + b"\0")

    for name, offset, ceiling in HEADER_COUNTS:
        if ceiling is None:
            if not fixture.rows["terminators"]:
                raise FixtureError("all-op fixture has no terminator for count relation")
            payload = bytearray(fixture.contents[:-TABLES["terminators"].row.size])
            struct.pack_into("<I", payload, 20, fixture.total - TABLES["terminators"].row.size)
            struct.pack_into("<I", payload, offset, fixture.counts[9] - 1)
            corpus.add("count-terminator-relation", 251,
                       "count.terminator-block-relation", 1, payload)
        else:
            corpus.u32(f"count-{name}-cap", 252, f"count.{name}-capacity", 0,
                       offset, ceiling + 1)

    combined = corpus.changed()
    struct.pack_into("<I", combined, 40, 896)
    struct.pack_into("<I", combined, 48, 3_201)
    corpus.add("count-combined-param-cap", 252, "count.combined-parameter-capacity", 1,
               combined)

    overwrite = corpus.changed()
    struct.pack_into("<I", overwrite, 24, 8_193)
    struct.pack_into("<I", overwrite, 20, 0)
    # The now-impossible extent is a later malformed relation.  Exhaustion must
    # remain 252 rather than being rewritten to 251.
    corpus.add("exhaustion-monotonic", 252, "status.252-not-overwritten-by-251", 1, overwrite)


def generate_dense_ids(corpus: Corpus) -> None:
    fixture = corpus.fixture
    for table in ID_TABLES:
        _, row = require_row(fixture, table)
        corpus.u32(f"dense-id-{table.replace('_', '-')}", 251, f"id.{table}.dense", 0,
                   fixture.row_offset(table, 0), row[0] + 1)


def generate_declarations(corpus: Corpus) -> None:
    f = corpus.fixture
    types, records, fields = f.rows["types"], f.rows["records"], f.rows["fields"]

    # Type tag, flags, reserved word, payload, range, interning, and nominal back relation.
    type_index, _ = require_row(f, "types")
    base = f.row_offset("types", type_index)
    corpus.u8("type-tag", 251, "type.tag", 1, base + 4, 0)
    corpus.u8("type-flags", 251, "type.flags", 0, base + 5, 2)
    corpus.u16("type-reserved", 251, "type.reserved", 0, base + 6, 1)
    bool_index, _ = find_row(f, "types", lambda row: row[1] == 3, "bool type")
    corpus.u8("type-forbidden-trapping", 251, "type.forbidden-trapping-flag", 0,
              f.row_offset("types", bool_index) + 5, 1)

    array_index, _ = find_row(f, "types", lambda row: row[1] == 5, "fixed-array type")
    corpus.u32("type-payload", 251, "type.payload", 1,
               f.row_offset("types", array_index) + 8, len(types))

    scalar_index, scalar = find_row(
        f, "types", lambda row: row[1] in (1, 2) and row[7] < 0x7FFF_FFFF,
        "bounded scalar type",
    )
    corpus.u32("type-range", 251, "type.range", 0,
               f.row_offset("types", scalar_index) + 16, scalar[7] + 1)

    scalar_rows = [(i, row) for i, row in enumerate(types) if row[1] in (1, 2)]
    if len(scalar_rows) < 2:
        raise FixtureError("all-op fixture lacks two scalar rows for interning tooth")
    source_index, _ = scalar_rows[0]
    target_index, _ = scalar_rows[1]
    interned = corpus.changed()
    source = f.row_offset("types", source_index)
    target = f.row_offset("types", target_index)
    interned[target + 4:target + 24] = f.contents[source + 4:source + 24]
    corpus.add("type-interning", 251, "type.interning", 1, interned)

    scalar_type = next(row[0] for row in types if row[1] in (1, 2, 3))
    corpus.u32("nominal-back-relation", 251, "type.nominal-record-relation", 1,
               f.row_offset("records", 0) + 4, scalar_type)

    # Declaration partitions, owners, and ordinals.
    record_index, record = find_row(f, "records", lambda row: row[3] > 0, "nonempty record")
    corpus.u8("record-flags", 251, "record.flags", 0,
              f.row_offset("records", record_index) + 16, 2)
    corpus.u8("record-reserved", 251, "record.reserved", 0,
              f.row_offset("records", record_index) + 17, 1)
    corpus.u32("record-field-span", 251, "span.record-fields", 0,
               f.row_offset("records", record_index) + 8, record[2] + 1)
    corpus.u32("record-field-count", 251, "span.record-field-count", 0,
               f.row_offset("records", record_index) + 12, 65)
    corpus.u32("field-owner", 251, "owner.field", 1,
               f.row_offset("fields", record[2]) + 4, len(records))
    corpus.u32("field-ordinal", 251, "ordinal.field", 0,
               f.row_offset("fields", record[2]) + 8, fields[record[2]][2] + 1)
    corpus.u32("field-type", 251, "type.field", 1,
               f.row_offset("fields", record[2]) + 12, len(types))

    machine_index, machine = find_row(
        f, "machines", lambda row: row[7] > 0, "machine with explicit parameters")
    corpus.u32("machine-owner", 251, "owner.machine", 0,
               f.row_offset("machines", 0) + 4, len(records))
    corpus.u8("machine-flags", 251, "machine.flags", 0,
              f.row_offset("machines", 0) + 9, 1)
    corpus.u16("machine-reserved", 251, "machine.reserved", 0,
               f.row_offset("machines", 0) + 10, 1)
    structural_type = next(row[0] for row in types if row[1] in (4, 5))
    corpus.u32("machine-result-structural", 251, "type.machine-result", 1,
               f.row_offset("machines", 0) + 12, structural_type)
    corpus.u32("machine-param-span", 251, "span.machine-parameters", 0,
               f.row_offset("machines", machine_index) + 16, machine[6] + 1)
    zero_param_machine_index, zero_param_machine = find_row(
        f, "machines", lambda row: row[7] == 0, "zero-parameter machine")
    corpus.u32("machine-param-zero-span", 251, "span.machine-parameters-zero", 0,
               f.row_offset("machines", zero_param_machine_index) + 16,
               zero_param_machine[6] + 1)
    corpus.u32("machine-param-count", 251, "span.machine-parameter-count", 0,
               f.row_offset("machines", machine_index) + 20, 8)
    corpus.u32("machine-block-span", 251, "span.machine-blocks", 1,
               f.row_offset("machines", 0) + 24, f.rows["machines"][0][8] + 1)
    corpus.u32("machine-block-count", 251, "span.machine-block-count", 0,
               f.row_offset("machines", 0) + 28, 129)
    first_machine = f.rows["machines"][0]
    alternate_entry = first_machine[8] + 1
    if alternate_entry >= first_machine[8] + first_machine[9]:
        raise FixtureError("all-op fixture lacks an alternate block for entry-ID tooth")
    corpus.u32("machine-entry-block", 251, "id.machine-entry-block", 1,
               f.row_offset("machines", 0) + 32, alternate_entry)
    corpus.u8("machine-receiver-access", 251, "receiver-access.machine", 0,
              f.row_offset("machines", 0) + 8, 0)

    mp_index, machine_param = require_row(f, "machine_params")
    corpus.u32("machine-param-owner", 251, "owner.machine-parameter", 0,
               f.row_offset("machine_params", mp_index) + 4, len(f.rows["machines"]))
    corpus.u32("machine-param-ordinal", 251, "ordinal.machine-parameter", 0,
               f.row_offset("machine_params", mp_index) + 8, machine_param[2] + 1)
    corpus.u32("machine-param-type", 251, "type.machine-parameter", 1,
               f.row_offset("machine_params", mp_index) + 12, structural_type)
    corpus.u32("machine-param-value", 251, "id.machine-parameter-value", 1,
               f.row_offset("machine_params", mp_index) + 16, machine_param[4] + 1)

    block_index, block = find_row(f, "blocks", lambda row: row[6] > 0, "block with parameters")
    corpus.u32("block-owner", 251, "owner.block", 0,
               f.row_offset("blocks", 0) + 4, len(f.rows["machines"]))
    corpus.u8("block-flags", 251, "block.flags", 0,
              f.row_offset("blocks", 0) + 9, 1)
    corpus.u16("block-reserved", 251, "block.reserved", 0,
               f.row_offset("blocks", 0) + 10, 1)
    corpus.u32("block-param-span", 251, "span.block-parameters", 0,
               f.row_offset("blocks", block_index) + 12, block[5] + 1)
    corpus.u32("block-param-count", 251, "span.block-parameter-count", 0,
               f.row_offset("blocks", block_index) + 16, 8)
    corpus.u32("block-operation-span", 251, "span.block-operations", 1,
               f.row_offset("blocks", 0) + 20, f.rows["blocks"][0][7] + 1)
    corpus.u32("block-terminator", 251, "id.block-terminator", 1,
               f.row_offset("blocks", 0) + 28, 1)
    corpus.u8("block-receiver-access", 251, "receiver-access.block", 0,
              f.row_offset("blocks", 0) + 8, 0)
    corpus.u8("entry-block-receiver-access", 251, "receiver-access.entry-block", 1,
              f.row_offset("blocks", first_machine[10]) + 8, 1)

    shared_machine = next((row for row in f.rows["machines"] if row[2] == 1), None)
    if shared_machine is None:
        raise FixtureError("all-op fixture lacks a shared-receiver machine")
    shared_block_index, _ = find_row(
        f, "blocks", lambda row: row[1] == shared_machine[0], "shared machine block")
    corpus.u8("block-access-exceeds-machine", 251, "receiver-access.block-exceeds-machine", 1,
              f.row_offset("blocks", shared_block_index) + 8, 2)

    bp_index, block_param = require_row(f, "block_params")
    corpus.u32("block-param-owner", 251, "owner.block-parameter", 0,
               f.row_offset("block_params", bp_index) + 4, len(f.rows["blocks"]))
    corpus.u32("block-param-ordinal", 251, "ordinal.block-parameter", 0,
               f.row_offset("block_params", bp_index) + 8, block_param[2] + 1)
    corpus.u32("block-param-type", 251, "type.block-parameter", 1,
               f.row_offset("block_params", bp_index) + 12, structural_type)
    corpus.u32("block-param-value", 251, "id.block-parameter-value", 1,
               f.row_offset("block_params", bp_index) + 16, block_param[4] + 1)


def generate_operations(corpus: Corpus) -> None:
    f = corpus.fixture
    operations = f.rows["operations"]
    operands = f.rows["operands"]
    types = f.rows["types"]

    op_index, op = find_row(f, "operations", lambda row: row[9] > 0, "operand-using operation")
    op_base = f.row_offset("operations", op_index)
    corpus.u16("operation-flags", 251, "operation.flags", 0,
               f.row_offset("operations", 0) + 14, 1)
    corpus.u32("operation-owner-machine", 251, "owner.operation-machine", 0,
               op_base + 4, len(f.rows["machines"]))
    corpus.u32("operation-owner-block", 251, "owner.operation-block", 0,
               op_base + 8, len(f.rows["blocks"]))
    corpus.u32("operation-operand-span", 251, "span.operation-operands", 1,
               op_base + 24, op[8] + 1)

    producing_index, producing = find_row(
        f, "operations", lambda row: row[4] in (1, 2), "result-producing operation")
    producing_base = f.row_offset("operations", producing_index)
    corpus.u8("operation-result-kind", 251, "operation.result-kind", 0,
              producing_base + 13, 3)
    corpus.u32("operation-result-id", 251, "operation.result-id", 1,
               producing_base + 16, NO_ID)
    corpus.u32("operation-result-type", 251, "operation.result-type", 0,
               producing_base + 20, len(types))
    corpus.u8("operation-opcode", 251, "operation.opcode", 1,
              producing_base + 12, 0)

    self_index, self_op = find_row(f, "operations", lambda row: row[3] == 2,
                                   "SelfPlace operation")
    alternate_nominal = next(
        row[0] for row in types
        if row[1] == 4 and row[0] != self_op[7]
    )
    corpus.u32("self-place-result-type", 251, "result.self-place-type", 1,
               f.row_offset("operations", self_index) + 20, alternate_nominal)

    const_result_index, const_result_op = find_row(
        f, "operations", lambda row: row[3] == 1, "Const operation")
    corpus.u32("const-result-type", 251, "result.const-scalar-type", 0,
               f.row_offset("operations", const_result_index) + 20,
               structural_type := next(row[0] for row in types if row[1] == 4))

    field_result_index, field_result_op = find_row(
        f, "operations", lambda row: row[3] == 3, "FieldPlace operation")
    field_wrong_type = next(row[0] for row in types if row[0] != field_result_op[7])
    corpus.u32("field-place-result-type", 251, "result.field-place-type", 0,
               f.row_offset("operations", field_result_index) + 20, field_wrong_type)

    index_result_index, index_result_op = find_row(
        f, "operations", lambda row: row[3] == 4, "IndexPlace operation")
    index_wrong_type = next(row[0] for row in types if row[0] != index_result_op[7])
    corpus.u32("index-place-result-type", 251, "result.index-place-type", 0,
               f.row_offset("operations", index_result_index) + 20, index_wrong_type)

    load_result_index, load_result_op = find_row(
        f, "operations", lambda row: row[3] == 5, "Load operation")
    load_wrong_type = next(row[0] for row in types if row[0] != load_result_op[7])
    corpus.u32("load-result-type", 251, "result.load-type", 0,
               f.row_offset("operations", load_result_index) + 20, load_wrong_type)

    add_result_index, add_result_op = find_row(
        f, "operations", lambda row: row[3] == 8, "Add operation")
    add_kind = types[add_result_op[7]][1]
    add_wrong_type = next(
        row[0] for row in types if row[1] in (1, 2) and row[1] != add_kind
    )
    corpus.u32("add-result-type", 251, "result.add-carrier", 1,
               f.row_offset("operations", add_result_index) + 20, add_wrong_type)

    less_index, less_op = find_row(f, "operations", lambda row: row[3] == 9,
                                   "Less operation")
    numeric_type = next(row[0] for row in types if row[1] in (1, 2))
    corpus.u32("less-result-type", 251, "result.less-bool", 1,
               f.row_offset("operations", less_index) + 20, numeric_type)

    no_result_index, no_result = find_row(
        f, "operations", lambda row: row[4] == 0, "no-result operation")
    corpus.u32("no-result-id", 251, "result.no-result-id", 1,
               f.row_offset("operations", no_result_index) + 16, 0)
    corpus.u32("no-result-type", 251, "result.no-result-type", 0,
               f.row_offset("operations", no_result_index) + 20, 0)

    # Insert one operand for an otherwise canonical Const while shifting every
    # later operation/terminator start.  The flat vector remains a complete,
    # contiguous partition, leaving operation arity as the sole bad relation.
    arity_index, arity_op = find_row(
        f, "operations", lambda row: row[3] == 1 and row[9] == 0,
        "zero-operand Const for arity tooth",
    )
    arity_operations = list(operations)
    changed = list(arity_operations[arity_index])
    changed[9] = 1
    arity_operations[arity_index] = tuple(changed)
    for index in range(arity_index + 1, len(arity_operations)):
        changed = list(arity_operations[index])
        changed[8] += 1
        arity_operations[index] = tuple(changed)
    arity_operands = list(operands)
    arity_operands.insert(arity_op[8], (0,))
    arity_terms = []
    for term in f.rows["terminators"]:
        changed = list(term)
        changed[8] += 1
        changed[11] += 1
        arity_terms.append(tuple(changed))
    corpus.add(
        "operation-arity", 251, "operation.arity", 1,
        f.reencode({
            "operations": arity_operations,
            "operands": arity_operands,
            "terminators": arity_terms,
        }),
    )

    value_count = f.counts[10]
    place_count = f.counts[11]
    corpus.u32("reconstructed-value-count", 251, "operation.reconstructed-value-count", 1,
               64, value_count - 1 if value_count else 1)
    corpus.u32("reconstructed-place-count", 251, "operation.reconstructed-place-count", 0,
               68, place_count - 1 if place_count else 1)

    # A non-Copy operation with a required-zero immediate.
    imm_index, imm_op = find_row(
        f, "operations", lambda row: row[3] != 7 and row[3] in range(1, 10),
        "operation with reserved immediate")
    corpus.u32("operation-immediate", 251, "operation.immediate", 0,
               f.row_offset("operations", imm_index) + 36, 1)
    corpus.u32("operation-immediate-zero", 251, "operation.immediate-zero", 0,
               f.row_offset("operations", self_index) + 32, 1)

    field_place_index, field_place = find_row(
        f, "operations", lambda row: row[3] == 3, "FieldPlace operation")
    corpus.u32("field-place-immediate", 251, "operation.field-immediate", 1,
               f.row_offset("operations", field_place_index) + 32,
               len(f.rows["fields"]))

    copy_index, copy_op = find_row(f, "operations", lambda row: row[3] == 7, "Copy operation")
    corpus.u32("copy-mode", 251, "operation.copy-mode", 1,
               f.row_offset("operations", copy_index) + 32, 3)

    copy_operands = f.operands_for(copy_op)
    copy_destination = f.places.get(copy_operands[0])
    copy_wrong_source = next(
        (
            place_id for place_id, info in f.places.items()
            if f.visible_place(info, copy_op)
            and copy_destination is not None
            and info.type_id != copy_destination.type_id
        ),
        None,
    )
    if copy_wrong_source is None:
        raise FixtureError("all-op fixture lacks an exact-type Copy mismatch")
    corpus.u32("copy-source-type", 251, "type.copy-source", 1,
               f.row_offset("operands", copy_op[8] + 1), copy_wrong_source)

    # Locate a value operand and replace it with an invalid dense reference.
    value_operand_site = None
    for candidate in operations:
        for ordinal, kind in enumerate(f.operand_kinds(candidate)):
            if kind == "value":
                value_operand_site = candidate[8] + ordinal
                break
        if value_operand_site is not None:
            break
    if value_operand_site is None:
        raise FixtureError("all-op fixture lacks a value operand")
    corpus.u32("operand-reference", 251, "operand.reference", 1,
               f.row_offset("operands", value_operand_site), value_count)

    place_operand_site = None
    place_use_before = None
    place_cross_block = None
    place_cross_machine = None
    for candidate in operations:
        for ordinal, kind in enumerate(f.operand_kinds(candidate)):
            if kind != "place":
                continue
            site = candidate[8] + ordinal
            if place_operand_site is None:
                place_operand_site = site
            if place_use_before is None and candidate[4] == 2:
                place_use_before = (site, candidate[6])
            for place_id, info in f.places.items():
                candidate_machine = candidate[1]
                place_machine = f.rows["blocks"][info.block][1]
                if (place_cross_block is None and place_machine == candidate_machine
                        and info.block != candidate[2]):
                    place_cross_block = (site, place_id)
                if place_cross_machine is None and place_machine != candidate_machine:
                    place_cross_machine = (site, place_id)
    if place_operand_site is None or place_use_before is None:
        raise FixtureError("all-op fixture lacks place reference/definition teeth")
    corpus.u32("place-reference", 251, "operand.place-reference", 1,
               f.row_offset("operands", place_operand_site), place_count)
    corpus.u32("place-use-before-definition", 251,
               "operand.place-use-before-definition", 1,
               f.row_offset("operands", place_use_before[0]), place_use_before[1])
    for label, mutation_class, site in (
        ("place-cross-block", "operand.place-cross-block", place_cross_block),
        ("place-cross-machine", "operand.place-cross-machine", place_cross_machine),
    ):
        if site is None:
            raise FixtureError(f"all-op fixture lacks a site for {mutation_class}")
        corpus.u32(label, 251, mutation_class, 0,
                   f.row_offset("operands", site[0]), site[1])

    # Add consumes carrier-compatible values and produces the same carrier, so
    # naming its own result is a type-correct, isolated use-before-definition.
    add_index, add_op = find_row(f, "operations", lambda row: row[3] == 8, "Add operation")
    corpus.u32("operand-use-before-definition", 251, "operand.use-before-definition", 1,
               f.row_offset("operands", add_op[8]), add_op[6])

    def value_operand_candidates(operation):
        for ordinal, kind in enumerate(f.operand_kinds(operation)):
            if kind == "value":
                yield ordinal, f.values.get(operands[operation[8] + ordinal][0])

    cross_block = None
    cross_machine = None
    wrong_type = None
    for candidate in operations:
        for ordinal, original in value_operand_candidates(candidate):
            if original is None:
                continue
            original_kind = types[original.type_id][1]
            for value_id, info in f.values.items():
                candidate_kind = types[info.type_id][1]
                if (cross_block is None and info.machine == candidate[1]
                        and info.block is not None and info.block != candidate[2]
                        and candidate_kind == original_kind):
                    cross_block = (candidate[8] + ordinal, value_id)
                if (cross_machine is None and info.machine != candidate[1]
                        and candidate_kind == original_kind):
                    cross_machine = (candidate[8] + ordinal, value_id)
                if (wrong_type is None and f.visible_value(info, candidate)
                        and candidate_kind != original_kind):
                    wrong_type = (candidate[8] + ordinal, value_id)
    for label, mutation_class, site in (
        ("operand-cross-block", "operand.cross-block", cross_block),
        ("operand-cross-machine", "operand.cross-machine", cross_machine),
        ("operand-type", "operand.type", wrong_type),
    ):
        if site is None:
            raise FixtureError(f"all-op fixture lacks a site for {mutation_class}")
        corpus.u32(label, 251, mutation_class, 1,
                   f.row_offset("operands", site[0]), site[1])

    # Static constant range failure.
    const_index, const_op = find_row(f, "operations", lambda row: row[3] == 1, "Const operation")
    const_type = types[const_op[7]]
    if const_type[7] < 0x7FFF_FFFF:
        out_of_range = const_type[7] + 1
    elif const_type[6] > 0:
        out_of_range = const_type[6] - 1
    else:
        raise FixtureError("all-op fixture has no Const with a mutable range tooth")
    corpus.u32("constant-range", 251, "operation.constant-range", 1,
               f.row_offset("operations", const_index) + 32, out_of_range)

    # Replace IndexPlace's array base with an earlier non-array place in the same block.
    index_index, index_op = find_row(f, "operations", lambda row: row[3] == 4, "IndexPlace operation")
    bad_base = next((place_id for place_id, info in f.places.items()
                     if f.visible_place(info, index_op) and types[info.type_id][1] != 5), None)
    if bad_base is None:
        raise FixtureError("all-op fixture lacks a non-array place before IndexPlace")
    corpus.u32("bad-index-place", 251, "operation.index-place-base", 1,
               f.row_offset("operands", index_op[8]), bad_base)

    # Keep the block structurally valid while making a receiver-derived Store
    # destination immutable.  Entry blocks cannot be used here because changing
    # their access would fail the earlier entry-signature relation instead.
    store_index, store_op = find_row(
        f, "operations",
        lambda row: row[3] == 6
        and row[2] != f.rows["machines"][row[1]][10]
        and f.rows["blocks"][row[2]][2] == 2,
        "Store operation in a non-entry mutable block",
    )
    corpus.u8("shared-receiver-store", 251, "operation.shared-receiver-store", 1,
              f.row_offset("blocks", store_op[2]) + 8, 1)

    # Remove the copy declaration from Copy's structural destination type.
    destination = f.places.get(copy_operands[0])
    if destination is None:
        raise FixtureError("Copy destination is not a reconstructed place")
    copy_type = destination.type_id
    while types[copy_type][1] == 5:
        copy_type = types[copy_type][4]
    if types[copy_type][1] != 4:
        raise FixtureError("Copy fixture does not reach a nominal copy type")
    copy_record = types[copy_type][4]
    corpus.u8("noncopyable-copy", 251, "operation.noncopyable-copy", 1,
              f.row_offset("records", copy_record) + 16, 0)


def generate_layout(corpus: Corpus) -> None:
    f = corpus.fixture
    record_index, record = find_row(f, "records", lambda row: row[3] > 0, "nonempty record")
    recursive = corpus.changed()
    struct.pack_into("<I", recursive, f.row_offset("fields", record[2]) + 12, record[1])
    corpus.add("recursive-layout", 251, "layout.recursive-by-value", 1, recursive)


def generate_terminators(corpus: Corpus) -> None:
    f = corpus.fixture
    terms = f.rows["terminators"]
    blocks = f.rows["blocks"]

    term_index, term = require_row(f, "terminators")
    base = f.row_offset("terminators", term_index)
    corpus.u8("terminator-kind", 251, "terminator.kind", 1, base + 12, 0)
    corpus.u8("terminator-flags", 251, "terminator.flags", 0, base + 13, 1)
    corpus.u16("terminator-reserved", 251, "terminator.reserved", 0,
               base + 14, 1)
    corpus.u32("terminator-owner-machine", 251, "owner.terminator-machine", 0,
               base + 4, len(f.rows["machines"]))
    corpus.u32("terminator-owner-block", 251, "owner.terminator-block", 0,
               base + 8, len(blocks))

    branch_index, branch = find_row(f, "terminators", lambda row: row[3] == 2, "Branch terminator")
    branch_base = f.row_offset("terminators", branch_index)
    corpus.u32("edge-operand-span", 251, "span.edge-operands", 1,
               branch_base + 24, branch[8] + 1)
    corpus.u32("edge-target1-span", 251, "span.edge-target1-operands", 0,
               branch_base + 36, branch[11] + 1)

    different_arity_target = next(
        (
            row[0] for row in blocks
            if row[1] == branch[1]
            and row[0] != f.rows["machines"][branch[1]][10]
            and row[6] != branch[9]
        ),
        None,
    )
    if different_arity_target is None:
        raise FixtureError("all-op fixture lacks a same-machine different-arity target")
    corpus.u32("edge-arity", 251, "edge.arity", 1,
               branch_base + 20, different_arity_target)

    if branch[9] == 0:
        raise FixtureError("all-op fixture lacks a nonempty edge argument vector")
    edge_operand_site = branch[8]
    target_parameter = f.rows["block_params"][blocks[branch[7]][5]]
    parameter_kind = f.rows["types"][target_parameter[3]][1]
    block_end = blocks[branch[2]][7] + blocks[branch[2]][8]

    def term_visible(info: ValueInfo) -> bool:
        return info.machine == branch[1] and (
            info.block is None
            or info.block == branch[2] and info.definition < block_end
        )

    invisible_edge_value = next(
        (
            value_id for value_id, info in f.values.items()
            if info.machine == branch[1]
            and info.block is not None
            and info.block != branch[2]
            and f.rows["types"][info.type_id][1] == parameter_kind
        ),
        None,
    )
    wrong_edge_type = next(
        (
            value_id for value_id, info in f.values.items()
            if term_visible(info)
            and f.rows["types"][info.type_id][1] != parameter_kind
        ),
        None,
    )
    if invisible_edge_value is None or wrong_edge_type is None:
        raise FixtureError("all-op fixture lacks isolated edge argument teeth")
    corpus.u32("edge-argument-visibility", 251, "operand.edge-visibility", 1,
               f.row_offset("operands", edge_operand_site), invisible_edge_value)
    corpus.u32("edge-argument-type", 251, "type.edge-argument", 1,
               f.row_offset("operands", edge_operand_site), wrong_edge_type)

    non_bool_condition = next(
        (
            value_id for value_id, info in f.values.items()
            if term_visible(info) and f.rows["types"][info.type_id][1] != 3
        ),
        None,
    )
    if non_bool_condition is None:
        raise FixtureError("all-op fixture lacks a visible non-bool branch value")
    corpus.u32("branch-condition-type", 251, "type.branch-condition", 1,
               branch_base + 16, non_bool_condition)
    corpus.u32("terminator-value-id", 251, "id.terminator-value", 0,
               branch_base + 16, f.counts[10])

    owner_machine = f.rows["machines"][branch[1]]
    corpus.u32("target-entry-block", 251, "target.entry-block", 1,
               branch_base + 20, owner_machine[10])
    corpus.u32("target-reference", 251, "target.reference", 1,
               branch_base + 20, len(blocks))
    other_block = next((row[0] for row in blocks if row[1] != branch[1]), None)
    if other_block is None:
        raise FixtureError("all-op fixture lacks a cross-machine target block")
    corpus.u32("target-cross-machine", 251, "target.cross-machine", 1,
               branch_base + 20, other_block)
    corpus.u32("target1-entry-block", 251, "target.target1-entry-block", 0,
               branch_base + 32, owner_machine[10])
    corpus.u32("target1-cross-machine", 251, "target.target1-cross-machine", 0,
               branch_base + 32, other_block)

    # Terminator class/shape teeth are deliberately based on existing Branch and
    # ReturnValue rows; no invented row IDs or fixture-specific offsets.
    corpus.u8("jump-shape", 251, "terminator.jump-shape", 1,
              branch_base + 12, 1)
    corpus.u32("branch-shape", 251, "terminator.branch-shape", 0,
               branch_base + 16, NO_ID)

    return_index, return_value = find_row(
        f, "terminators", lambda row: row[3] == 4, "ReturnValue terminator")
    return_base = f.row_offset("terminators", return_index)
    corpus.u8("return-unit-shape", 251, "terminator.return-unit-shape", 1,
              return_base + 12, 3)
    corpus.u32("return-value-shape", 251, "terminator.return-value-shape", 0,
               return_base + 16, NO_ID)
    return_owner = f.rows["machines"][return_value[1]]
    return_target = next(
        row[0] for row in blocks
        if row[1] == return_value[1]
        and row[0] != return_owner[10]
        and row[6] == 0
    )
    corpus.u32("return-target-shape", 251, "terminator.return-target-shape", 0,
               return_base + 20, return_target)

    bool_type = next(row[0] for row in f.rows["types"] if row[1] == 3)
    corpus.u32("return-carrier", 251, "type.return-carrier", 1,
               f.row_offset("machines", return_value[1]) + 12, bool_type)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("manifest", type=Path, nargs="?")
    args = parser.parse_args()

    fixture = Fixture(args.input.read_bytes())
    args.output_dir.mkdir(parents=True, exist_ok=True)
    manifest = args.manifest or args.output_dir / "manifest.tsv"
    manifest.parent.mkdir(parents=True, exist_ok=True)
    corpus = Corpus(fixture, args.output_dir, manifest)

    generate_header(corpus)
    generate_dense_ids(corpus)
    generate_declarations(corpus)
    generate_operations(corpus)
    generate_layout(corpus)
    generate_terminators(corpus)
    corpus.finish()
    print(f"generated {len(corpus.rows)} CKIR1 mutations in {args.output_dir}")


if __name__ == "__main__":
    try:
        main()
    except (FixtureError, OSError, struct.error) as error:
        raise SystemExit(f"checked IR mutations: {error}")
