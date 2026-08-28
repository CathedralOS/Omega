#!/usr/bin/env python3
"""Independent OMGCOMP1/OMGRSWA fixed-buffer source relation."""

from __future__ import annotations

import dataclasses
import re
import struct
import sys
from pathlib import Path

from omgrfn18_u64 import U64
from omgrfn21_frame import RefinementError, RefinementResourceError, require

HERE = Path(__file__).resolve().parent
COMPILER = HERE.parents[2] / "source/on-ramp/omega-bootstrap/compiler"
sys.path.insert(0, str(COMPILER))
import omega_bootstrap_compilation as compilation  # noqa: E402

NO_ID = 0xFFFF_FFFF
HEADER = struct.Struct("<8sHHHH28I")
ROWS = {
    "units": struct.Struct("<5I"),
    "types": struct.Struct("<IBBH6I"),
    "records": struct.Struct("<7I"),
    "fields": struct.Struct("<6I"),
    "machines": struct.Struct("<14I"),
    "params": struct.Struct("<6I"),
    "blocks": struct.Struct("<10I"),
    "calls": struct.Struct("<9I"),
}
ORDER = tuple(ROWS)
CEILINGS = {
    "units": 16, "types": 2048, "records": 128, "fields": 4096,
    "machines": 128, "params": 2048, "blocks": 4096, "calls": 4096,
}


@dataclasses.dataclass(frozen=True)
class Witness:
    raw: bytes
    counts: dict[str, int]
    tables: dict[str, tuple[tuple[int, ...], ...]]
    offsets: dict[str, tuple[int, int]]
    input_length: int
    selected_root: int
    selected_record: int
    clear_machine: int
    append_machine: int
    lookup_machine: int
    length: int
    u8_type: int
    bool_type: int
    index_type: int
    length_type: int
    array_type: int
    buffer_type: int
    root_type: int


@dataclasses.dataclass(frozen=True)
class SourceModel:
    source: bytes
    length: int
    array_field: str
    length_field: str
    status_field: str
    byte_parameter: str
    index_parameter: str
    clear_machine: str
    append_machine: str
    lookup_machine: str
    root_machine: str
    result: int


def _span(source: bytes, start: int, length: int, label: str) -> bytes:
    require(start <= len(source) and length <= len(source) - start, label)
    return source[start:start + length]


def _identifier(source: bytes, start: int, length: int, label: str) -> str:
    raw = _span(source, start, length, label)
    require(re.fullmatch(rb"[A-Za-z_][A-Za-z0-9_]*", raw) is not None, label)
    return raw.decode("ascii")


def _u64(row: tuple[int, ...]) -> tuple[U64, U64]:
    return U64(row[4], row[5]), U64(row[6], row[7])


def decode_witness(raw: bytes) -> Witness:
    require(len(raw) >= HEADER.size, "truncated OMGRSWA header")
    head = HEADER.unpack_from(raw)
    magic, major, minor, flags, header_size, *words = head
    require((magic, major, minor, flags, header_size) ==
            (b"OMGRSWA\0", 10, 0, 0, HEADER.size), "exact OMGRSWA header")
    require(words[0] == len(raw), "OMGRSWA exact length")
    counts = dict(zip(ORDER, words[2:10]))
    for name, count in counts.items():
        if count > CEILINGS[name]:
            raise RefinementResourceError(f"OMGRSWA {name} ceiling")
    require(words[23] == 1 and words[24:] == [0, 0, 0, 0],
            "complete OMGRSWA relation flags/reserved")
    at = HEADER.size
    tables: dict[str, tuple[tuple[int, ...], ...]] = {}
    offsets: dict[str, tuple[int, int]] = {}
    for name in ORDER:
        row = ROWS[name]
        count = counts[name]
        extent = row.size * count
        require(extent <= len(raw) - at, f"OMGRSWA {name} extent")
        offsets[name] = (at, extent)
        tables[name] = tuple(row.unpack_from(raw, at + row.size * index)
                             for index in range(count))
        require(all(item[0] == index for index, item in enumerate(tables[name])),
                f"OMGRSWA dense {name} IDs")
        at += extent
    require(at == len(raw), "OMGRSWA exact EOF")
    return Witness(raw, counts, tables, offsets, words[1], *words[10:23])


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
    envelope, source = source_closure(omgcomp)
    witness = decode_witness(raw)
    require(witness.input_length == len(omgcomp), "OMGRSWA paired OMGCOMP extent")
    require(witness.counts["units"] == 1, "one complete source unit")
    unit = witness.tables["units"][0]
    require(unit == (0, 0, 0, len(source), 0), "complete source-unit custody")
    require(1 <= witness.length <= 65_536, "public fixed-array length")

    types = witness.tables["types"]
    for type_id in (witness.u8_type, witness.bool_type, witness.index_type,
                    witness.length_type, witness.array_type, witness.buffer_type,
                    witness.root_type):
        require(type_id < len(types), "selected normalized type ID")
    require(types[witness.u8_type] ==
            (witness.u8_type, 1, 0, 0, 0, 0, 0, 0, 255, 0),
            "canonical u8 type")
    require(types[witness.bool_type] ==
            (witness.bool_type, 3, 0, 0, 0, 0, 0, 0, 1, 0),
            "canonical bool type")
    require(types[witness.index_type] ==
            (witness.index_type, 10, 1, 0, 0, 0, 0, 0,
             0xFFFF_FFFF, 0xFFFF_FFFF),
            "authored trapping full-u64 lookup index")
    require(types[witness.length_type] ==
            (witness.length_type, 10, 0, 0, 0, 0, 0, 0,
             witness.length, 0),
            "authored exact length-u64 interval and policy")
    require(types[witness.array_type] ==
            (witness.array_type, 5, 1, 0, witness.u8_type, witness.length,
             0, 0, 0, 0),
            "authored trapping fixed-byte array")
    require(types[witness.buffer_type][1:4] == (4, 0, 0)
            and types[witness.buffer_type][4] == witness.selected_record,
            "buffer nominal type")

    records, fields = witness.tables["records"], witness.tables["fields"]
    require(witness.selected_record < len(records), "selected buffer record")
    record = records[witness.selected_record]
    require(record[1] == 0 and record[2] == witness.buffer_type and record[4] == 3,
            "SourceUnit-like record field partition")
    selected_fields = fields[record[3]:record[3] + record[4]]
    require(len(selected_fields) == 3
            and {row[3] for row in selected_fields} ==
                {witness.array_type, witness.length_type, witness.bool_type},
            "array/length/status field family")
    field_names = {row[3]: _identifier(source, row[4], row[5], "authored field span")
                   for row in selected_fields}

    machines = witness.tables["machines"]
    for machine_id in (witness.clear_machine, witness.append_machine,
                       witness.lookup_machine, witness.selected_root):
        require(machine_id < len(machines), "selected machine ID")
    clear, append, lookup, root = (machines[witness.clear_machine],
                                   machines[witness.append_machine],
                                   machines[witness.lookup_machine],
                                   machines[witness.selected_root])
    require(clear[2] == append[2] == lookup[2] == witness.selected_record,
            "buffer-machine ownership")
    require((clear[3], clear[4], clear[6], clear[8]) == (2, NO_ID, 0, 1),
            "clear signature and entry")
    require((append[3], append[4], append[6], append[8]) == (2, NO_ID, 1, 3),
            "append signature and states")
    require((lookup[3], lookup[4], lookup[6], lookup[8]) ==
            (1, witness.u8_type, 1, 3), "lookup signature and states")
    require(root[4] == witness.u8_type and root[6] == 0 and root[8] == 1,
            "root harness signature")
    names = [_identifier(source, row[9], row[10], "authored machine span")
             for row in machines]
    params = witness.tables["params"]
    require(len(params) == 2, "exact append/lookup parameters")
    append_param = next((row for row in params if row[1] == witness.append_machine), None)
    lookup_param = next((row for row in params if row[1] == witness.lookup_machine), None)
    require(append_param is not None and append_param[3] == witness.u8_type,
            "append byte parameter")
    require(lookup_param is not None and lookup_param[3] == witness.index_type,
            "lookup trapping full-u64 parameter")
    byte_name = _identifier(source, append_param[4], append_param[5], "append parameter span")
    index_name = _identifier(source, lookup_param[4], lookup_param[5], "lookup parameter span")

    blocks = witness.tables["blocks"]
    require(all(row[1] < len(machines) and row[3] == machines[row[1]][3]
                and row[4] == row[5] == 0 for row in blocks),
            "state ownership/access and zero parameters")
    require(all(row[11] <= len(source) and row[12] <= len(source) - row[11]
                for row in machines), "machine body spans")
    calls = witness.tables["calls"]
    require(len(calls) == 5 and all(row[1] == 0 and row[2] == witness.selected_root
                                   and row[8] == 0 for row in calls),
            "root-only ordinary calls")
    require([row[3] for row in calls] == [witness.clear_machine,
                                         witness.append_machine,
                                         witness.lookup_machine,
                                         witness.append_machine,
                                         witness.lookup_machine],
            "complete root call order")
    root_body_start, root_body_length = root[11], root[12]
    for row in calls:
        require(row[4] < len(fields), "call receiver field")
        receiver = fields[row[4]]
        require(receiver[1] == records[types[witness.root_type][4]][0]
                and receiver[3] == witness.buffer_type, "root buffer receiver custody")
        invocation = _span(source, row[5], row[6], "authored call span").decode("ascii")
        require(root_body_start <= row[5]
                and row[6] <= root_body_start + root_body_length - row[5],
                "call span is inside selected root body")
        require(re.fullmatch(rf"{re.escape(names[row[3]])}\s*\([^()]*\)", invocation,
                             re.S) is not None, "authored call target identity")

    model = _check_source_semantics(source, witness, names, field_names,
                                    byte_name, index_name)
    require(envelope.root_source_id == 0,
            "selected root source custody")
    root_owner = envelope.strings[envelope.root_owner_string_id]
    root_machine = envelope.strings[envelope.root_machine_string_id]
    root_record = records[types[witness.root_type][4]]
    require(_identifier(source, root_record[5], root_record[6], "root owner span") == root_owner
            and names[witness.selected_root] == root_machine,
            "selected OMGCOMP root identity")
    return witness, model


def _compact(raw: bytes) -> str:
    text = raw.decode("ascii")
    text = re.sub(r"//[^\n]*|/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"\s+", "", text)


def _check_source_semantics(source: bytes, witness: Witness, names: list[str],
                            fields: dict[int, str], byte_name: str,
                            index_name: str) -> SourceModel:
    machines = witness.tables["machines"]
    body = lambda mid: _compact(_span(source, machines[mid][11], machines[mid][12],
                                     "authored machine body"))
    array_name = fields[witness.array_type]
    length_name = fields[witness.length_type]
    status_name = fields[witness.bool_type]
    clear = body(witness.clear_machine)
    append = body(witness.append_machine)
    lookup = body(witness.lookup_machine)
    root = body(witness.selected_root)
    n = witness.length
    whole = _compact(source)

    require(whole.count(f"{array_name}:[u8;{n}]inTrapping;") == 1,
            "authored trapping fixed-byte-array spelling")
    require(whole.count(f"{length_name}:u64[0..={n}];") == 1
            and f"{length_name}:u64inTrapping" not in whole,
            "authored unqualified constrained-length spelling")
    require(whole.count(f"{status_name}:bool;") == 1,
            "authored Boolean status spelling")
    require(re.search(
        rf"machine[A-Za-z_]\w*::{re.escape(names[witness.append_machine])}"
        rf"\(&mutself,{re.escape(byte_name)}:u8\)", whole) is not None,
        "authored append byte signature")
    require(re.search(
        rf"machine[A-Za-z_]\w*::{re.escape(names[witness.lookup_machine])}"
        rf"\(&self,{re.escape(index_name)}:u64inTrapping\)->u8", whole) is not None
            and whole.count("u64inTrapping") == 1,
        "authored trapping full-u64 lookup signature")

    require(clear.count(f"self.{length_name}=0;") == 1
            and clear.count(f"self.{status_name}=true;") == 1,
            "clear exact length/status initialization")
    require(append.count(f"self.{status_name}=false;") == 2
            and append.count(f"self.{status_name}=true;") == 1,
            "append success/failure status")
    require(append.count(f"transition self.{length_name}<{n}".replace(" ", "")) == 1,
            "append direct retained-length guard")
    require(append.count(f"self.{array_name}[self.{length_name}]={byte_name};") == 1,
            "guarded direct full-u64 IndexPlace Store source")
    require(append.count(f"self.{length_name}=self.{length_name}+1;") == 1,
            "direct exact u64 leaf-plus-literal increment")
    arithmetic = append.replace("->", "")
    require(arithmetic.count("+") == 1 and arithmetic.count("<") == 1
            and not any(token in arithmetic for token in ("-", "*", "/")),
            "append excludes unrelated arithmetic and relations")
    append_indexes = re.findall(r"\[([^\]]+)\]", append)
    require(append_indexes == [f"self.{length_name}"],
            "append index is direct and effect-free")

    require(lookup.count(f"transition{index_name}<self.{length_name}") == 1,
            "lookup direct full-u64 guard")
    require(lookup.count(f"self.{array_name}[{index_name}]") == 1,
            "guarded direct full-u64 IndexPlace Load source")
    require(re.findall(r"\[([^\]]+)\]", lookup) == [index_name]
            and lookup.count("<") == 1 and "+" not in lookup,
            "lookup excludes computed/effectful index and unrelated u64 ops")

    source_field = next(row for row in witness.tables["fields"]
                        if row[1] == witness.tables["records"][
                            witness.tables["types"][witness.root_type][4]][0]
                        and row[3] == witness.buffer_type)
    source_name = _identifier(source, source_field[4], source_field[5],
                              "root buffer field span")
    clear_name, append_name = names[witness.clear_machine], names[witness.append_machine]
    lookup_name, root_name = names[witness.lookup_machine], names[witness.selected_root]
    expected_calls = (
        f"self.{source_name}.{clear_name}();",
        f"self.{source_name}.{append_name}(70);",
        f"self.{source_name}.{lookup_name}(0)",
        f"self.{source_name}.{length_name}={n};",
        f"self.{source_name}.{append_name}(71);",
        f"self.{source_name}.{lookup_name}({n})",
    )
    require(all(root.count(piece) == 1 for piece in expected_calls),
            "finite canonical root exercise")
    # The accepted source proof: true append edge gives length <= N-1, hence
    # both IndexPlace and exact +1 are safe; lookup's true edge gives index <
    # length <= N.  The root observes retained byte 70 and the full-path miss.
    require(n >= 1, "nonempty fixed array proof")
    return SourceModel(source, n, array_name, length_name, status_name,
                       byte_name, index_name, clear_name, append_name,
                       lookup_name, root_name, 70)


def parse_selected_source(omgcomp: bytes, witness_raw: bytes) -> SourceModel:
    return check_witness_relation(omgcomp, witness_raw)[1]
