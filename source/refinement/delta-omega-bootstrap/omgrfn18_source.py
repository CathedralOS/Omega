#!/usr/bin/env python3
"""Independent OMGRSW8 and direct pure u64-Less source relation."""

from __future__ import annotations

import dataclasses
import re
import struct
import sys
from pathlib import Path

from omgrfn18_frame import RefinementError, RefinementResourceError, require
from omgrfn18_u64 import FULL_HIGH, FULL_LOW, U64

HERE = Path(__file__).resolve().parent
COMPILER = HERE.parents[2] / "source/on-ramp/omega-bootstrap/compiler"
sys.path.insert(0, str(COMPILER))
import omega_bootstrap_compilation as compilation  # noqa: E402

WIDTHS = (("units", 36), ("imports", 48), ("bindings", 28),
          ("declarations", 28), ("types", 24), ("records", 24),
          ("fields", 24), ("sums", 24), ("cases", 28), ("payloads", 24),
          ("machines", 40), ("machine_parameters", 24),
          ("blocks", 40), ("block_parameters", 24))
COUNT_NAMES = ("sources", "imports", "bindings", "declarations", "types",
               "records", "fields", "machines", "machine_parameters",
               "blocks", "block_parameters", "sums", "cases", "payloads",
               "selected", "reserved")
CEILINGS = (16, 64, 4096, 256, 2048, 128, 4096, 128, 2048, 2048,
            4096, 128, 512, 4096)
TYPE_ROW = struct.Struct("<IBBHIIII")
U64_KIND = 10


@dataclasses.dataclass(frozen=True)
class Witness:
    counts: dict[str, int]
    rows: dict[str, tuple[bytes, ...]]
    types: tuple[tuple[int, ...], ...]


@dataclasses.dataclass(frozen=True)
class DeclaredType:
    low: U64
    high: U64


@dataclasses.dataclass(frozen=True)
class LessSite:
    subject: str
    ceiling: U64
    true_target: str
    true_arguments: tuple[str, ...]
    false_target: str
    false_arguments: tuple[str, ...]
    fact_low: U64
    fact_high: U64


@dataclasses.dataclass(frozen=True)
class SourceModel:
    source: bytes
    fields: dict[str, DeclaredType]
    parameters: dict[str, dict[str, DeclaredType]]
    less: LessSite
    stored_value: U64
    true_result: int
    false_result: int

    @property
    def result(self) -> int:
        return self.true_result if self.stored_value.less(self.less.ceiling) else self.false_result


def _span(source: bytes, start: int, length: int) -> bytes:
    require(start <= len(source) and length <= len(source) - start,
            "authored source span")
    return source[start:start + length]


def source_closure(omgcomp: bytes) -> tuple[object, tuple[bytes, ...]]:
    try:
        envelope = compilation.decode(omgcomp)
    except compilation.CompilationError as error:
        if getattr(error, "status", 251) == 252:
            raise RefinementResourceError(str(error)) from error
        raise RefinementError(str(error)) from error
    sources = tuple(envelope.bundle_entries[row.bundle_entry_id].content
                    for row in envelope.sources)
    return envelope, sources


def decode_witness(raw: bytes) -> Witness:
    require(len(raw) >= 84, "truncated OMGRSW8 header")
    require(raw[:8] == b"OMGRSW8\0"
            and struct.unpack_from("<4H", raw, 8) == (8, 0, 0, 84),
            "exact OMGRSW8 header")
    words = struct.unpack_from("<17I", raw, 16)
    require(words[0] == len(raw), "OMGRSW8 exact length")
    counts = dict(zip(COUNT_NAMES, words[1:]))
    require(counts["reserved"] == 0, "OMGRSW8 reserved")
    for name, ceiling in zip(COUNT_NAMES[:14], CEILINGS):
        if counts[name] > ceiling:
            raise RefinementResourceError(f"OMGRSW8 {name} ceiling")
    ordered_counts = (counts["sources"], counts["imports"], counts["bindings"],
                      counts["declarations"], counts["types"], counts["records"],
                      counts["fields"], counts["sums"], counts["cases"],
                      counts["payloads"], counts["machines"],
                      counts["machine_parameters"], counts["blocks"],
                      counts["block_parameters"])
    at = 84
    rows: dict[str, tuple[bytes, ...]] = {}
    for (name, width), count in zip(WIDTHS, ordered_counts):
        require(count <= (len(raw) - at) // width, f"OMGRSW8 {name} extent")
        rows[name] = tuple(raw[at + width * i:at + width * (i + 1)]
                           for i in range(count))
        at += width * count
        require(all(struct.unpack_from("<I", row)[0] == i
                    for i, row in enumerate(rows[name])),
                f"OMGRSW8 dense {name} IDs")
    require(at == len(raw), "OMGRSW8 exact EOF")
    types = tuple(TYPE_ROW.unpack(row) for row in rows["types"])
    for row in types:
        _, kind, flags, reserved, low_lo, low_hi, high_lo, high_hi = row
        require(reserved == 0, "normalized type reserved")
        if kind == U64_KIND:
            require(flags == 0, "unqualified u64 policy")
            require(not U64(high_lo, high_hi).less(U64(low_lo, low_hi)),
                    "normalized u64 interval")
    return Witness(counts, rows, types)


def check_witness_relation(omgcomp: bytes, raw: bytes) -> Witness:
    envelope, sources = source_closure(omgcomp)
    witness = decode_witness(raw)
    require(witness.counts["sources"] == len(sources),
            "source-closure cardinality")
    seen_sources: set[int] = set()
    for unit in witness.rows["units"]:
        source_id = struct.unpack_from("<I", unit, 4)[0]
        start, length = struct.unpack_from("<II", unit, 12)
        require(source_id < len(sources), "unit source ID")
        if start != 0xFFFF_FFFF:
            require(re.fullmatch(rb"[A-Za-z_][A-Za-z0-9_]*",
                                 _span(sources[source_id], start, length)) is not None,
                    "unit module identity span")
        else:
            require(length == 0, "absent unit module span")
        seen_sources.add(source_id)
    require(seen_sources == set(range(len(sources))),
            "complete source-unit custody")

    declarations: list[tuple[int, bytes]] = []
    for row in witness.rows["declarations"]:
        source_id = struct.unpack_from("<I", row, 8)[0]
        start, length = struct.unpack_from("<II", row, 16)
        require(source_id < len(sources), "declaration source ID")
        name = _span(sources[source_id], start, length)
        require(re.fullmatch(rb"[A-Za-z_][A-Za-z0-9_]*", name) is not None,
                "declaration authored identity")
        declarations.append((source_id, name))
    selected = witness.counts["selected"]
    require(selected < len(witness.rows["machines"]), "selected machine ID")
    machine = witness.rows["machines"][selected]
    declaration_id, record_id = struct.unpack_from("<II", machine, 4)
    require(declaration_id < len(declarations)
            and record_id < len(witness.rows["records"]),
            "selected machine ownership")
    owner_decl = struct.unpack_from("<I", witness.rows["records"][record_id], 4)[0]
    require(owner_decl < len(declarations), "selected owner declaration")
    root_owner = envelope.strings[envelope.root_owner_string_id].encode("ascii")
    root_machine = envelope.strings[envelope.root_machine_string_id].encode("ascii")
    require(declarations[declaration_id] == (envelope.root_source_id, root_machine),
            "selected machine equals OMGCOMP root")
    require(declarations[owner_decl] == (envelope.root_source_id, root_owner),
            "selected owner equals OMGCOMP root")

    u64_types = {row[0] for row in witness.types if row[1] == U64_KIND}
    require(bool(u64_types), "OMGRSW8 selected u64 type")
    require(any(row[1:] == (U64_KIND, 0, 0, 0, 0,
                            0xFFFF_FFFF, 0xFFFF_FFFF)
                for row in witness.types), "OMGRSW8 exact full-u64 row")
    for table in ("fields", "machine_parameters", "block_parameters"):
        for row in witness.rows[table]:
            type_id = struct.unpack_from("<I", row, 12)[0]
            require(type_id < len(witness.types), f"{table} normalized type ID")

    records = [struct.unpack_from("<5I", row) for row in witness.rows["records"]]
    machines = [struct.unpack_from("<3I", row) for row in witness.rows["machines"]]
    blocks = [struct.unpack_from("<3I", row) for row in witness.rows["blocks"]]
    linked: dict[str, list[tuple[int, ...]]] = {}
    for row in witness.rows["fields"]:
        _, owner, _, type_id, start, length = struct.unpack("<6I", row)
        require(owner < len(records), "field owner")
        source_id = declarations[records[owner][1]][0]
        name = _span(sources[source_id], start, length).decode("ascii")
        linked.setdefault(name, []).append(witness.types[type_id])
    for table, owners, block_owned in (
        ("machine_parameters", machines, False),
        ("block_parameters", blocks, True),
    ):
        for row in witness.rows[table]:
            _, owner, _, type_id, start, length = struct.unpack("<6I", row)
            require(owner < len(owners), f"{table} owner")
            machine_id = blocks[owner][1] if block_owned else owner
            source_id = declarations[machines[machine_id][1]][0]
            name = _span(sources[source_id], start, length).decode("ascii")
            linked.setdefault(name, []).append(witness.types[type_id])
    require(any(row[1] == U64_KIND for row in linked.get("stored", ())),
            "authored stored field resolves to u64")
    require(any(row[1] == U64_KIND for row in linked.get("value", ())),
            "authored transported parameter resolves to u64")
    selected_text = sources[envelope.root_source_id].decode("ascii")
    for match in re.finditer(
        r"([A-Za-z_]\w*)\s*:\s*u64(?:\s*\[([0-9]+\s*\.\.=\s*[0-9]+)\])?",
        selected_text,
    ):
        name = match.group(1)
        if match.group(2) is None:
            expected = (0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF)
        else:
            low_raw, high_raw = (int(piece) for piece in
                                 re.sub(r"\s+", "", match.group(2)).split("..="))
            low_value, high_value = U64.from_int(low_raw), U64.from_int(high_raw)
            expected = (low_value.lo, low_value.hi, high_value.lo, high_value.hi)
        require(any(row[1:4] == (U64_KIND, 0, 0) and row[4:] == expected
                    for row in linked.get(name, ())),
                "authored u64 declaration to normalized endpoint words")
    return witness


def _declared_type(raw: str | None) -> DeclaredType:
    if raw is None:
        return DeclaredType(FULL_LOW, FULL_HIGH)
    low, high = (int(piece) for piece in raw.split("..="))
    result = DeclaredType(U64.from_int(low), U64.from_int(high))
    require(not result.high.less(result.low), "authored u64 interval")
    return result


def _parameters(raw: str) -> dict[str, DeclaredType]:
    result: dict[str, DeclaredType] = {}
    for item in raw.split(","):
        item = item.strip()
        if not item or re.fullmatch(r"&\s*(?:mut\s+)?self", item):
            continue
        match = re.fullmatch(
            r"([A-Za-z_]\w*)\s*:\s*u64(?:\s*\[([0-9]+\.\.=\s*[0-9]+)\])?", item,
        )
        require(match is not None, "direct unqualified u64 parameter")
        result[match.group(1)] = _declared_type(
            None if match.group(2) is None else re.sub(r"\s+", "", match.group(2))
        )
    return result


def parse_selected_source(omgcomp: bytes) -> SourceModel:
    envelope, sources = source_closure(omgcomp)
    source = sources[envelope.root_source_id]
    try:
        text = source.decode("ascii")
    except UnicodeDecodeError as error:
        raise RefinementError("selected source ASCII") from error
    text = re.sub(r"//[^\n]*|/\*.*?\*/", "", text, flags=re.S)
    require("u64 in Trapping" not in text, "u64 is unqualified")

    fields: dict[str, DeclaredType] = {}
    data = re.search(r"\bdata\s+[A-Za-z_]\w*\s*\{(.*?)\}", text, re.S)
    require(data is not None, "selected owner data declaration")
    for match in re.finditer(
        r"([A-Za-z_]\w*)\s*:\s*u64(?:\s*\[([0-9]+\s*\.\.=\s*[0-9]+)\])?\s*;", data.group(1)
    ):
        fields[match.group(1)] = _declared_type(
            None if match.group(2) is None else re.sub(r"\s+", "", match.group(2))
        )
    require(bool(fields), "selected u64 field")

    parameters: dict[str, dict[str, DeclaredType]] = {}
    for match in re.finditer(
        r"(?:state\s+|machine\s+[A-Za-z_]\w*::)([A-Za-z_]\w*)\s*\(([^)]*)\)", text
    ):
        parameters[match.group(1)] = _parameters(match.group(2))

    assignments = re.findall(r"self\.([A-Za-z_]\w*)\s*=\s*([0-9]+)\s*;", text)
    require(len(assignments) == 1 and assignments[0][0] in fields,
            "one direct u64 literal initialization")
    stored_name, magnitude = assignments[0]
    stored_value = U64.from_int(int(magnitude))
    require(stored_value.in_closed(fields[stored_name].low, fields[stored_name].high),
            "initialized field interval")

    transition = re.search(
        r"transition\s+(self\.[A-Za-z_]\w*|[A-Za-z_]\w*)\s*<\s*([0-9]+)\s*\{\s*"
        r"true\s*->\s*([A-Za-z_]\w*)\(([^)]*)\)\s*"
        r"false\s*->\s*([A-Za-z_]\w*)\(([^)]*)\)\s*\}", text, re.S,
    )
    require(transition is not None, "one direct pure u64 Less transition")
    subject, ceiling_raw, yes, yes_raw, no, no_raw = transition.groups()
    subject_name = subject[5:] if subject.startswith("self.") else subject
    require(subject.startswith("self.") and subject_name in fields,
            "Less subject is direct typed field load")
    ceiling = U64.from_int(int(ceiling_raw))
    require(ceiling != FULL_LOW, "true edge requires a predecessor")
    yes_args = tuple(part.strip() for part in yes_raw.split(",") if part.strip())
    no_args = tuple(part.strip() for part in no_raw.split(",") if part.strip())
    require(yes_args.count(subject) == 1 and len(yes_args) == 1,
            "true edge forwards the direct Less subject once")
    require(yes in parameters and no in parameters, "transition target declarations")
    require(len(parameters[yes]) == 1 and not parameters[no] and not no_args,
            "focused true/false target arity")
    target_type = next(iter(parameters[yes].values()))
    target_name = next(iter(parameters[yes]))
    subject_type = fields[subject_name]
    fact_low = subject_type.low
    predecessor = ceiling.predecessor()
    fact_high = predecessor if predecessor.less(subject_type.high) else subject_type.high
    require(not fact_high.less(fact_low), "reachable true-edge intersection")
    require(fact_low.in_closed(target_type.low, target_type.high)
            and fact_high.in_closed(target_type.low, target_type.high),
            "true-edge fact authorizes target parameter interval")

    target_state = re.search(
        rf"\bstate\s+{re.escape(yes)}\s*\([^)]*\)\s*\{{(.*?)\}}", text, re.S
    )
    require(target_state is not None, "true target body")
    transport = re.search(
        r"self\.([A-Za-z_]\w*)\s*=\s*self\.([A-Za-z_]\w*)\s*\(\s*"
        r"([A-Za-z_]\w*)\s*\)\s*;", target_state.group(1), re.S,
    )
    require(transport is not None
            and transport.group(1) == subject_name
            and transport.group(3) == target_name,
            "direct target-identity Call result to storage")
    helper_name = transport.group(2)
    helper = re.search(
        rf"\bmachine\s+[A-Za-z_]\w*::{re.escape(helper_name)}\s*\(([^)]*)\)\s*"
        r"->\s*u64(?:\s*\[([0-9]+\s*\.\.=\s*[0-9]+)\])?\s*"
        r"\{\s*([A-Za-z_]\w*)\s*\}", text, re.S,
    )
    require(helper is not None and helper_name in parameters,
            "direct u64 echo machine")
    helper_params = parameters[helper_name]
    require(len(helper_params) == 1 and helper.group(3) in helper_params,
            "echo returns its direct parameter identity")
    helper_type = helper_params[helper.group(3)]
    helper_result = _declared_type(
        None if helper.group(2) is None else re.sub(r"\s+", "", helper.group(2))
    )
    require(helper_type == helper_result == target_type,
            "same constrained carrier across edge, Call, and result")

    def state_result(name: str) -> int:
        state = re.search(rf"\bstate\s+{re.escape(name)}\s*\([^)]*\)\s*\{{(.*?)\}}", text, re.S)
        require(state is not None, f"{name} state body")
        direct = re.search(r"(?:^|;)\s*([0-9]+)\s*$", state.group(1))
        require(direct is not None and 0 <= int(direct.group(1)) <= 255,
                f"{name} direct u8 result")
        return int(direct.group(1))

    return SourceModel(
        source, fields, parameters,
        LessSite(subject, ceiling, yes, yes_args, no, no_args, fact_low, fact_high),
        stored_value, state_result(yes), state_result(no),
    )
