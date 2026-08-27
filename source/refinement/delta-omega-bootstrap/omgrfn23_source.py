#!/usr/bin/env python3
"""Independent OMGCOMP1/OMGRSWC12 full TokenStream source relation."""

from __future__ import annotations

import dataclasses
import re
import struct
import sys
from pathlib import Path

from omgrfn23_frame import RefinementError, RefinementResourceError, require

HERE = Path(__file__).resolve().parent
COMPILER = HERE.parents[3] / "source/on-ramp/omega-bootstrap/compiler"
sys.path.insert(0, str(COMPILER))
import omega_bootstrap_compilation as compilation  # noqa: E402

NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH44I")
ORDER = ("units", "types", "records", "fields", "sums", "cases",
         "payloads", "machines", "params", "blocks", "block_params",
         "calls", "stores", "store_paths", "arguments")
ROWS = {
    "units": struct.Struct("<5I"),
    "types": struct.Struct("<IBBH6I"),
    "records": struct.Struct("<8I"),
    "fields": struct.Struct("<6I"),
    "sums": struct.Struct("<8I"),
    "cases": struct.Struct("<7I"),
    "payloads": struct.Struct("<6I"),
    "machines": struct.Struct("<14I"),
    "params": struct.Struct("<6I"),
    "blocks": struct.Struct("<10I"),
    "block_params": struct.Struct("<6I"),
    "calls": struct.Struct("<9I"),
    "stores": struct.Struct("<10I"),
    "store_paths": struct.Struct("<I"),
    "arguments": struct.Struct("<8I"),
}
COUNTS = dict(zip(ORDER, (1, 22, 8, 29, 5, 105, 8, 3, 11, 14, 11,
                          2, 15, 20, 11)))
CEILINGS = {
    "units": 16, "types": 2048, "records": 128, "fields": 4096,
    "sums": 128, "cases": 4096, "payloads": 4096, "machines": 128,
    "params": 2048, "blocks": 4096, "block_params": 4096,
    "calls": 4096, "stores": 4096, "store_paths": 16_384,
    "arguments": 65_536,
}


@dataclasses.dataclass(frozen=True)
class Witness:
    raw: bytes
    counts: dict[str, int]
    tables: dict[str, tuple[tuple[int, ...], ...]]
    offsets: dict[str, tuple[int, int]]
    input_length: int
    root_machine: int
    record_ids: tuple[int, ...]
    sum_ids: tuple[int, ...]
    float_case: int
    push_machine: int
    read_machine: int
    token_capacity: int
    decoded_capacity: int
    owner_size: int
    owner_ceiling: int


@dataclasses.dataclass(frozen=True)
class SourceModel:
    source: bytes
    record_names: tuple[str, ...]
    sum_names: tuple[str, ...]
    field_names: tuple[str, ...]
    machine_names: tuple[str, ...]
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
    return re.sub(rb"\s+", b"", raw)


def decode_witness(raw: bytes) -> Witness:
    require(len(raw) >= HEADER.size, "truncated OMGRSWC12 header")
    magic, major, minor, flags, header_size, *words = HEADER.unpack_from(raw)
    require((magic, major, minor, flags, header_size) ==
            (b"OMGRSWC\0", 12, 0, 0, HEADER.size),
            "exact OMGRSWC12 header")
    require(words[0] == len(raw), "OMGRSWC12 exact length")
    counts = dict(zip(ORDER, words[2:17]))
    for name, count in counts.items():
        if count > CEILINGS[name]:
            raise RefinementResourceError(f"OMGRSWC12 {name} ceiling")
    require(words[36] == 1 and words[38] == 0
            and words[39] == 2 * 1024 * 1024 and words[40:] == [0] * 4,
            "complete relation/resource header")
    cursor = HEADER.size
    tables: dict[str, tuple[tuple[int, ...], ...]] = {}
    offsets: dict[str, tuple[int, int]] = {}
    for name in ORDER:
        row, count = ROWS[name], counts[name]
        extent = row.size * count
        require(extent <= len(raw) - cursor, f"OMGRSWC12 {name} extent")
        offsets[name] = (cursor, extent)
        tables[name] = tuple(row.unpack_from(raw, cursor + row.size * index)
                             for index in range(count))
        if name != "store_paths":
            require(all(item[0] == index for index, item in enumerate(tables[name])),
                    f"OMGRSWC12 dense {name} IDs")
        cursor += extent
    require(cursor == len(raw), "OMGRSWC12 exact EOF")
    return Witness(raw, counts, tables, offsets, words[1], words[17],
                   tuple(words[18:26]), tuple(words[26:31]), words[31],
                   words[32], words[33], words[34], words[35], words[37], words[39])


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


def _partition(rows, owners: int, starts: tuple[int, ...], counts: tuple[int, ...],
               label: str) -> None:
    require(len(starts) == owners and len(counts) == owners, label)
    cursor = 0
    for owner, (start, count) in enumerate(zip(starts, counts)):
        require(start == cursor and all(row[1] == owner and row[2] == ordinal
                                        for ordinal, row in enumerate(
                                            rows[start:start + count])), label)
        cursor += count
    require(cursor == len(rows), label)


def check_witness_relation(omgcomp: bytes, raw: bytes) -> tuple[Witness, SourceModel]:
    _, source = source_closure(omgcomp)
    witness = decode_witness(raw)
    require(len(raw) == 7_520 and witness.input_length == len(omgcomp),
            "exact OMGRSWC12/paired OMGCOMP extent")
    require(witness.counts == COUNTS, "complete normalized OMGRSWC12 census")
    unit = witness.tables["units"][0]
    require(unit[0] == 0 and unit[3:] == (len(source), 0)
            and _span(omgcomp, unit[1], unit[2], "unit module span") == b"app",
            "complete source-unit/module custody")
    require((witness.root_machine, witness.record_ids, witness.sum_ids,
             witness.float_case, witness.push_machine, witness.read_machine,
             witness.token_capacity, witness.decoded_capacity,
             witness.owner_size, witness.owner_ceiling) ==
            (2, tuple(range(8)), tuple(range(5)), 78, 0, 1,
             16_384, 65_536, 1_638_456, 2 * 1024 * 1024),
            "selected source identities and owner/resource edge")

    types = witness.tables["types"]
    require([row[0] for row in types] == list(range(22)), "dense type IDs")
    require([row[1] for row in types[:13]] == [4] * 8 + [6] * 5,
            "eight record and five sum nominal types")
    require([row[4] for row in types[:8]] == list(range(8))
            and [row[4] for row in types[8:13]] == list(range(5)),
            "nominal type payload identities")
    require([row[1] for row in types[13:19]] == [1, 2, 3, 10, 10, 10],
            "u8/u32/bool/full-u64/Exact carrier family")
    require(types[14][2] == 1 and types[16][2] == 1,
            "authored trapping u32/full-u64 policy")
    require([row[1] for row in types[19:]] == [5, 5, 5]
            and [(row[2], row[4], row[5]) for row in types[19:]] ==
            [(1, 3, 16_384), (1, 5, 16_384), (1, 13, 65_536)],
            "trapping Token/Observation/decoded arrays")

    records, fields = witness.tables["records"], witness.tables["fields"]
    require([(row[0], row[1], row[2], row[3], row[4], row[7]) for row in records] ==
            [(rid, 0, rid, start, count, int(rid <= 5))
             for rid, (start, count) in enumerate(
                 ((0, 1), (1, 2), (3, 2), (5, 4), (9, 2),
                  (11, 9), (20, 8), (28, 1)))],
            "record partitions and nested copy policy")
    field_types = (14, 16, 16, 0, 1, 11, 2, 16, 16, 12, 2,
                   13, 13, 13, 13, 14, 16, 16, 16, 16,
                   19, 20, 17, 21, 18, 4, 15, 15, 6)
    starts = tuple(row[3] for row in records); counts = tuple(row[4] for row in records)
    _partition(fields, 8, starts, counts, "record field partition")
    require(tuple(row[3] for row in fields) == field_types,
            "exact normalized field carriers")

    sums, cases, payloads = (witness.tables[name]
                             for name in ("sums", "cases", "payloads"))
    case_counts = (4, 30, 42, 9, 20)
    case_starts = (0, 4, 34, 76, 85)
    require([(row[0], row[1], row[2], row[3], row[4], row[7]) for row in sums] ==
            [(sid, 0, 8 + sid, case_starts[sid], case_counts[sid], 1)
             for sid in range(5)], "sum partitions and copy policy")
    _partition(cases, 5, case_starts, case_counts, "sum case partition")
    require([row[4] for row in cases[76:85]] == [0, 3, 3, 0, 1, 1, 0, 0, 0],
            "TokenKind exact payload arities")
    require([(row[1], row[2], row[3]) for row in payloads] ==
            [(77, 0, 8), (77, 1, 15), (77, 2, 15),
             (78, 0, 15), (78, 1, 15), (78, 2, 15),
             (80, 0, 9), (81, 0, 10)],
            "Integer/Float/Keyword/Punctuation payload identities")

    machines = witness.tables["machines"]
    require([(row[0], row[1], row[2], row[3], row[4], row[5], row[6],
              row[7], row[12], row[13]) for row in machines] ==
            [(0, 2, 6, NO_ID, 0, 10, 0, 3, 0, 0),
             (1, 1, 6, 13, 10, 1, 3, 10, 0, 0),
             (2, 2, 7, 13, 11, 0, 13, 1, 0, 0)],
            "push/read_kind/root machine signatures")
    params = witness.tables["params"]
    require([(row[1], row[2], row[3]) for row in params] ==
            [(0, ordinal, type_id) for ordinal, type_id in enumerate(
                (0, 11, 16, 16, 16, 16, 13, 13, 13, 13))] + [(1, 0, 16)],
            "10-argument push with two structural parameters")
    blocks = witness.tables["blocks"]
    require([(row[1], row[2], row[3]) for row in blocks] ==
            [(0, ordinal, 2) for ordinal in range(3)] +
            [(1, ordinal, 1) for ordinal in range(10)] + [(2, 0, 2)],
            "exact push/read_kind/run block ownership")
    block_params = witness.tables["block_params"]
    require([(row[1], row[2], row[3]) for row in block_params] ==
            [(owner, sum(1 for prior in (5, 5, 5, 6, 6, 6, 7, 8, 9, 9, 10)[:pid]
                        if prior == owner), type_id)
             for pid, (owner, type_id) in enumerate(zip(
                 (5, 5, 5, 6, 6, 6, 7, 8, 9, 9, 10),
                 (8, 15, 15, 15, 15, 15, 9, 10, 15, 15, 15)))],
            "complete Float/discard/control binders")

    calls = witness.tables["calls"]
    require([(row[0], row[1], row[2], row[3], row[4], row[7], row[8])
             for row in calls] == [(0, 2, 13, 0, NO_ID, 0, 10),
                                    (1, 2, 13, 1, 13, 10, 1)],
            "real push/read_kind calls through Main.stream")
    for row in calls:
        require(_span(source, row[5], row[6], "call span").endswith(b")"),
                "call span exact closing delimiter")

    stores = witness.tables["stores"]
    paths = tuple(row[0] for row in witness.tables["store_paths"])
    require(sum(row[5] for row in stores) == 20
            and all(row[4] <= len(paths) and row[5] <= len(paths) - row[4]
                    for row in stores), "15-store exact path partition")
    expected_paths = ((5,), (6, 3), (6, 4, 1), (6, 4, 2), (7,), (8,),
                      (11,), (12,), (13,), (14,), (15,), (16,), (17,),
                      (18,), (19,))
    require(tuple(paths[row[4]:row[4] + row[5]] for row in stores) == expected_paths,
            "whole kind/source and 13 scalar destination paths")
    require([(row[1], row[2], row[3], row[9]) for row in stores] ==
            [(0, 1, 20 if index < 6 else 21, 22) for index in range(15)],
            "all data stores use guarded current TokenStream index")
    require([(row[6], row[7], row[8]) for row in stores] ==
            [(1, 11, 0), (0, 0, 0), (2, 16, 0), (3, 16, 0),
             (4, 16, 0), (5, 16, 0), (6, 13, 0), (7, 13, 0),
             (8, 13, 0), (9, 13, 0), (0, 14, 1), (2, 16, 0),
             (3, 16, 0), (4, 16, 0), (5, 16, 0)],
            "two structural, source.value, and scalar source identities")
    arguments = witness.tables["arguments"]
    require([(row[1], row[2], row[3]) for row in arguments] ==
            [(0, ordinal, type_id) for ordinal, type_id in enumerate(
                (0, 11, 16, 16, 16, 16, 13, 13, 13, 13))] + [(1, 0, 16)],
            "exact call argument contextual carriers")
    require([(row[4], row[5], row[6], row[7]) for row in arguments] ==
            [(1, 4, 0, 0), (2, 78, 5, 0)] +
            [(0, value, 0, 0) for value in (5, 6, 7, 8, 70, 1, 2, 3, 0)],
            "SourceId/Float constructors and exact literal arguments")

    record_names = tuple(_identifier(source, row[5], row[6], "record name")
                         for row in records)
    sum_names = tuple(_identifier(source, row[5], row[6], "sum name")
                      for row in sums)
    field_names = tuple(_identifier(source, row[4], row[5], "field name")
                        for row in fields)
    machine_names = tuple(_identifier(source, row[8], row[9], "machine name")
                          for row in machines)
    parameter_names = tuple(_identifier(source, row[4], row[5], "parameter name")
                            for row in params)
    require(all(row[10] <= len(source) and row[11] <= len(source) - row[10]
                for row in machines), "machine body spans")
    require(all(row[8] <= len(source) and row[9] <= len(source) - row[8]
                for row in blocks), "block body spans")
    compact = _compact(source)
    for name in (*record_names[:6], *sum_names):
        require(b"data" + name.encode() + b"[copy]{" in compact,
                "authored recursive [copy] custody")
    require(b"data" + record_names[6].encode() + b"{" in compact
            and b"data" + record_names[7].encode() + b"{" in compact,
            "stream/root remain noncopy")
    fn = [name.encode() for name in field_names]
    require(fn[20] + b":[" + record_names[3].encode() + b";16384]inTrapping;"
            in compact and fn[21] + b":[" + record_names[5].encode()
            + b";16384]inTrapping;" in compact,
            "authored trapping aggregate arrays")
    require(fn[22] + b":u64[0..=16384];" in compact,
            "authored Exact token count")
    push_body = _compact(_span(source, machines[0][10], machines[0][11], "push body"))
    count, retained = fn[22], fn[27]
    require(b"self." + count + b"<16384" in push_body
            and b"self." + count + b"=self." + count + b"+1;" in push_body,
            "exact capacity guard and increment")
    require(push_body.startswith(b"{self." + retained + b"=false;")
            and push_body.count(b"self." + retained + b"=false;") == 2
            and push_body.count(b"self." + retained + b"=true;") == 1,
            "last_retained status discipline")
    read_body = _compact(_span(source, machines[1][10], machines[1][11],
                               "read_kind body"))
    require(b"::Float{has_exponent,empty_exponent,has_suffix}" in read_body
            and b"has_exponent{true->" in read_body
            and b"empty_exponent{true->" in read_body
            and b"has_suffix{true->" in read_body,
            "Float payload readback and true/false/true decision chain")
    run_body = _compact(_span(source, machines[2][10], machines[2][11], "run body"))
    require(b"{value:4}" in run_body
            and b"::Float{has_exponent:true,empty_exponent:false,has_suffix:true}"
            in run_body and b"5,6,7,8,70,1,2,3" in run_body,
            "joint Float(true,false,true) and observation-tag-70 source result")
    return witness, SourceModel(source, record_names, sum_names, field_names,
                                machine_names, parameter_names, 70)


def source_result(omgcomp: bytes, witness: bytes) -> int:
    return check_witness_relation(omgcomp, witness)[1].result
