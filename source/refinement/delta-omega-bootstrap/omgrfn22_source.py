#!/usr/bin/env python3
"""Independent OMGCOMP1/OMGRSWB flat record-array source relation."""

from __future__ import annotations

import dataclasses
import re
import struct
import sys
from pathlib import Path

from omgrfn22_frame import RefinementError, RefinementResourceError, require

HERE = Path(__file__).resolve().parent
COMPILER = HERE.parents[3] / "source/on-ramp/omega-bootstrap/compiler"
sys.path.insert(0, str(COMPILER))
import omega_bootstrap_compilation as compilation  # noqa: E402

NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH36I")
ORDER = ("units", "types", "records", "fields", "machines", "params",
         "blocks", "calls", "stores", "arguments")
ROWS = {
    "units": struct.Struct("<5I"),
    "types": struct.Struct("<IBBH6I"),
    "records": struct.Struct("<8I"),
    "fields": struct.Struct("<6I"),
    "machines": struct.Struct("<14I"),
    "params": struct.Struct("<6I"),
    "blocks": struct.Struct("<10I"),
    "calls": struct.Struct("<9I"),
    "stores": struct.Struct("<8I"),
    "arguments": struct.Struct("<6I"),
}
CEILINGS = {"units": 16, "types": 2048, "records": 128, "fields": 4096,
            "machines": 128, "params": 2048, "blocks": 4096,
            "calls": 4096, "stores": 4096, "arguments": 65536}


@dataclasses.dataclass(frozen=True)
class Witness:
    raw: bytes
    counts: dict[str, int]
    tables: dict[str, tuple[tuple[int, ...], ...]]
    offsets: dict[str, tuple[int, int]]
    input_length: int
    selected_root: int
    observation_record: int
    stream_record: int
    root_record: int
    push_machine: int
    lookup_machine: int
    length: int
    u8_type: int
    u32_type: int
    bool_type: int
    index_type: int
    count_type: int
    array_type: int
    observation_type: int
    stream_type: int
    root_type: int


@dataclasses.dataclass(frozen=True)
class SourceModel:
    source: bytes
    record_names: tuple[str, str, str]
    field_names: tuple[str, ...]
    machine_names: tuple[str, str, str]
    parameter_names: tuple[str, ...]
    result: int


def _span(source: bytes, start: int, length: int, label: str) -> bytes:
    require(start <= len(source) and length <= len(source) - start, label)
    return source[start:start + length]


def _identifier(source: bytes, start: int, length: int, label: str) -> str:
    raw = _span(source, start, length, label)
    require(re.fullmatch(rb"[A-Za-z_][A-Za-z0-9_]*", raw) is not None, label)
    return raw.decode("ascii")


def _compact(raw: bytes) -> bytes:
    previous = None
    while previous != raw:
        previous = raw
        raw = re.sub(rb"/\*[^*]*?(?:\*(?!/)[^*]*?)*\*/", b"", raw,
                     flags=re.DOTALL)
    raw = re.sub(rb"//[^\n]*", b"", raw)
    raw = re.sub(rb"\s+", b"", raw)
    return raw


def decode_witness(raw: bytes) -> Witness:
    require(len(raw) >= HEADER.size, "truncated OMGRSWB header")
    magic, major, minor, flags, header_size, *words = HEADER.unpack_from(raw)
    require((magic, major, minor, flags, header_size) ==
            (b"OMGRSWB\0", 11, 0, 0, HEADER.size), "exact OMGRSWB header")
    require(words[0] == len(raw), "OMGRSWB exact length")
    counts = dict(zip(ORDER, words[2:12]))
    for name, count in counts.items():
        if count > CEILINGS[name]:
            raise RefinementResourceError(f"OMGRSWB {name} ceiling")
    require(words[28] == 1 and words[29:] == [0] * 7,
            "complete OMGRSWB relation flags/reserved")
    cursor = HEADER.size
    tables: dict[str, tuple[tuple[int, ...], ...]] = {}
    offsets: dict[str, tuple[int, int]] = {}
    for name in ORDER:
        row, count = ROWS[name], counts[name]
        extent = row.size * count
        require(extent <= len(raw) - cursor, f"OMGRSWB {name} extent")
        offsets[name] = (cursor, extent)
        tables[name] = tuple(row.unpack_from(raw, cursor + row.size * index)
                             for index in range(count))
        require(all(item[0] == index for index, item in enumerate(tables[name])),
                f"OMGRSWB dense {name} IDs")
        cursor += extent
    require(cursor == len(raw), "OMGRSWB exact EOF")
    return Witness(raw, counts, tables, offsets, words[1], *words[12:28])


def source_closure(omgcomp: bytes) -> tuple[object, bytes]:
    try:
        envelope = compilation.decode(omgcomp)
    except compilation.CompilationError as error:
        if getattr(error, "status", 251) == 252:
            raise RefinementResourceError(f"OMGCOMP1 source closure: {error}") from error
        raise RefinementError(f"OMGCOMP1 source closure: {error}") from error
    require(len(envelope.sources) == 1, "exact one-unit source closure")
    row = envelope.sources[0]
    return envelope, envelope.bundle_entries[row.bundle_entry_id].content


def check_witness_relation(omgcomp: bytes, raw: bytes) -> tuple[Witness, SourceModel]:
    _, source = source_closure(omgcomp)
    witness = decode_witness(raw)
    require(witness.input_length == len(omgcomp), "OMGRSWB paired OMGCOMP extent")
    require(witness.counts == {"units": 1, "types": 10, "records": 3,
            "fields": 13, "machines": 3, "params": 10, "blocks": 7,
            "calls": 2, "stores": 9, "arguments": 9},
            "complete normalized OMGRSWB census")
    require(witness.tables["units"] == ((0, 0, 0, len(source), 0),),
            "complete source-unit custody")
    require((witness.selected_root, witness.observation_record,
             witness.stream_record, witness.root_record, witness.push_machine,
             witness.lookup_machine, witness.length) == (2, 0, 1, 2, 0, 1, 16_384),
            "selected source identities")
    require((witness.u8_type, witness.u32_type, witness.bool_type,
             witness.index_type, witness.count_type, witness.array_type,
             witness.observation_type, witness.stream_type, witness.root_type) ==
            tuple(range(1, 10)), "selected normalized type identities")

    types = witness.tables["types"]
    require(types == (
        (0, 0, 0, 0, 0, 0, 0, 0, 0, 0),
        (1, 1, 0, 0, 0, 0, 0, 0, 255, 0),
        (2, 2, 1, 0, 0, 0, 0, 0, 0xFFFF_FFFF, 0),
        (3, 3, 0, 0, 0, 0, 0, 0, 1, 0),
        (4, 10, 1, 0, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF),
        (5, 10, 0, 0, 0, 0, 0, 0, 16_384, 0),
        (6, 5, 1, 0, 7, 16_384, 0, 0, 0, 0),
        (7, 4, 0, 0, 0, 0, 0, 0, 0, 0),
        (8, 4, 0, 0, 1, 0, 0, 0, 0, 0),
        (9, 4, 0, 0, 2, 0, 0, 0, 0, 0),
    ), "authored scalar policy, endpoints, array, and nominal types")

    records, fields = witness.tables["records"], witness.tables["fields"]
    require([(row[0], row[1], row[2], row[3], row[4], row[7]) for row in records] ==
            [(0, 0, 7, 0, 9, 1), (1, 0, 8, 9, 3, 0),
             (2, 0, 9, 12, 1, 0)], "record partitions and copy policy")
    record_names = tuple(_identifier(source, row[5], row[6], "record name span")
                         for row in records)
    require([(row[0], row[1], row[2], row[3]) for row in fields] ==
            [(index, 0, index, (1, 1, 1, 1, 2, 4, 4, 4, 4)[index])
             for index in range(9)] +
            [(9, 1, 0, 6), (10, 1, 1, 5), (11, 1, 2, 3), (12, 2, 0, 8)],
            "normalized field owner/ordinal/type relation")
    field_names = tuple(_identifier(source, row[4], row[5], "field name span")
                        for row in fields)

    machines = witness.tables["machines"]
    require([(row[0], row[1], row[2], row[3], row[4], row[5], row[6],
              row[7], row[8], row[13]) for row in machines] ==
            [(0, 0, 1, 2, NO_ID, 0, 9, 0, 3, 0),
             (1, 0, 1, 1, 1, 9, 1, 3, 3, 0),
             (2, 0, 2, 2, 1, 10, 0, 6, 1, 0)],
            "push/read/root machine signatures")
    machine_names = tuple(_identifier(source, row[9], row[10], "machine name span")
                          for row in machines)
    require(all(row[11] <= len(source) and row[12] <= len(source) - row[11]
                for row in machines), "machine body spans")

    params = witness.tables["params"]
    require([(row[0], row[1], row[2], row[3]) for row in params] ==
            [(index, 0, index, (1, 1, 1, 1, 2, 4, 4, 4, 4)[index])
             for index in range(9)] + [(9, 1, 0, 4)],
            "push/read parameter carriers and order")
    parameter_names = tuple(_identifier(source, row[4], row[5], "parameter span")
                            for row in params)

    blocks = witness.tables["blocks"]
    require([(row[0], row[1], row[2], row[3], row[4], row[5]) for row in blocks] ==
            [(0, 0, 0, 2, 0, 0), (1, 0, 1, 2, 0, 0),
             (2, 0, 2, 2, 0, 0), (3, 1, 0, 1, 0, 0),
             (4, 1, 1, 1, 0, 0), (5, 1, 2, 1, 0, 0),
             (6, 2, 0, 2, 0, 0)], "block ownership/access/ordinal relation")
    require(all(row[8] <= len(source) and row[9] <= len(source) - row[8]
                for row in blocks), "block body spans")

    calls = witness.tables["calls"]
    require([(row[0], row[1], row[2], row[3], row[4], row[7], row[8])
             for row in calls] ==
            [(0, 0, 2, 0, 12, 9, 0), (1, 0, 2, 1, 12, 1, 0)],
            "two direct root calls through stream field")
    for row in calls:
        require(_span(source, row[5], row[6], "call span").endswith(b")"),
                "call span ends at matching close")

    stores = witness.tables["stores"]
    require(stores == tuple((index, 0, 1, 9, 10, index, index,
                             (1, 1, 1, 1, 2, 4, 4, 4, 4)[index])
                            for index in range(9)),
            "nine direct field-store paths and parameter bijection")
    arguments = witness.tables["arguments"]
    require(arguments == tuple((index, 0, index,
                                (1, 1, 1, 1, 2, 4, 4, 4, 4)[index],
                                (70, 1, 2, 3, 4, 5, 6, 7, 8)[index], 0)
                               for index in range(9)),
            "nine pure scalar root-call literals")

    compact = _compact(source)
    obs, stream, root = (name.encode() for name in record_names)
    fn = [name.encode() for name in field_names]
    mn = [name.encode() for name in machine_names]
    pn = [name.encode() for name in parameter_names]
    require(b"data" + obs + b"[copy]{" in compact,
            "authored Observation copy marker custody")
    require(b"data" + stream + b"{" in compact
            and b"data" + root + b"{" in compact,
            "stream/root remain authored noncopy records")
    # Exact authored policy is independently visible in declaration text.
    for name in fn[:4]:
        require(name + b":u8;" in compact, "authored u8 observation field")
    require(fn[4] + b":u32inTrapping;" in compact,
            "authored trapping u32 observation field")
    for name in fn[5:9]:
        require(name + b":u64inTrapping;" in compact,
                "authored trapping u64 observation field")
    require(fn[9] + b":[" + obs + b";16384]inTrapping;" in compact,
            "authored trapping record array")
    require(fn[10] + b":u64[0..=16384];" in compact,
            "authored constrained count")
    require(fn[11] + b":bool;" in compact and fn[12] + b":" + stream + b";" in compact,
            "status and root-stream fields")

    push_body = _compact(_span(source, machines[0][11], machines[0][12], "push body"))
    require(b"transition" + b"self." + fn[10] + b"<16384{true->" in push_body,
            "direct count < N guard and true arm")
    for index in range(9):
        assignment = (b"self." + fn[9] + b"[self." + fn[10] + b"]." + fn[index]
                      + b"=" + pn[index] + b";")
        require(push_body.count(assignment) == 1,
                "one direct guarded store per observation field")
    require(push_body.count(b"self." + fn[10] + b"=self." + fn[10] + b"+1;") == 1,
            "authored Exact count plus literal one")
    lookup_body = _compact(_span(source, machines[1][11], machines[1][12], "lookup body"))
    require(b"transition" + pn[9] + b"<self." + fn[10] + b"{true->" in lookup_body,
            "direct read-index < count guard")
    require((b"self." + fn[9] + b"[" + pn[9] + b"]." + fn[0]) in lookup_body,
            "guarded direct tag readback path")
    root_body = _compact(_span(source, machines[2][11], machines[2][12], "root body"))
    require((b"self." + fn[12] + b"." + mn[0] + b"(70,1,2,3,4,5,6,7,8)")
            in root_body and (b"self." + fn[12] + b"." + mn[1] + b"(0)") in root_body,
            "root exact push/read calls and observations")
    return witness, SourceModel(source, record_names, field_names, machine_names,
                                parameter_names, 70)


def source_result(omgcomp: bytes, raw: bytes) -> int:
    _, model = check_witness_relation(omgcomp, raw)
    return model.result
