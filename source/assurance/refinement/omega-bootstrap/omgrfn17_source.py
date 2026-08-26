#!/usr/bin/env python3
"""Small source/OMGRSW4 model for the generalized-view refinement cut."""

from __future__ import annotations

import dataclasses
import re
import struct
import sys
from pathlib import Path

from omgrfn17_frame import RefinementError, RefinementResourceError, require

HERE = Path(__file__).resolve().parent
COMPILER = HERE.parents[3] / "bootstrap/omega-bootstrap/compiler"
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


@dataclasses.dataclass(frozen=True)
class Witness:
    counts: dict[str, int]
    rows: dict[str, tuple[bytes, ...]]


@dataclasses.dataclass(frozen=True)
class Guard:
    view: str
    true_target: str
    false_target: str
    head_index: int
    tail_index: int
    passthrough: tuple[str, ...]


@dataclasses.dataclass(frozen=True)
class SourceModel:
    source: bytes
    guards: tuple[Guard, ...]
    literal: bytes | None
    result: int


def _span(source: bytes, start: int, length: int) -> bytes:
    require(start <= len(source) and length <= len(source) - start, "authored source span")
    return source[start:start + length]


def decode_witness(raw: bytes) -> Witness:
    require(len(raw) >= 84, "truncated OMGRSW4 header")
    require(raw[:8] == b"OMGRSW4\0" and struct.unpack_from("<4H", raw, 8) == (4, 0, 0, 84),
            "exact OMGRSW4 header")
    words = struct.unpack_from("<17I", raw, 16)
    require(words[0] == len(raw), "OMGRSW4 exact length")
    counts = dict(zip(COUNT_NAMES, words[1:]))
    require(counts["reserved"] == 0, "OMGRSW4 reserved")
    for name, ceiling in zip(COUNT_NAMES[:14], CEILINGS):
        if counts[name] > ceiling:
            raise RefinementResourceError(f"OMGRSW4 {name} ceiling")
    ordered_counts = (counts["sources"], counts["imports"], counts["bindings"],
                      counts["declarations"], counts["types"], counts["records"],
                      counts["fields"], counts["sums"], counts["cases"],
                      counts["payloads"], counts["machines"],
                      counts["machine_parameters"], counts["blocks"],
                      counts["block_parameters"])
    at = 84; rows: dict[str, tuple[bytes, ...]] = {}
    for (name, width), count in zip(WIDTHS, ordered_counts):
        require(count <= (len(raw) - at) // width, f"OMGRSW4 {name} extent")
        rows[name] = tuple(raw[at + width * i:at + width * (i + 1)] for i in range(count))
        at += width * count
        require(all(struct.unpack_from("<I", row)[0] == i for i, row in enumerate(rows[name])),
                f"OMGRSW4 dense {name} IDs")
    require(at == len(raw), "OMGRSW4 exact EOF")
    return Witness(counts, rows)


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


def check_witness_relation(omgcomp: bytes, raw: bytes) -> Witness:
    envelope, sources = source_closure(omgcomp)
    witness = decode_witness(raw)
    require(witness.counts["sources"] == len(sources), "source-closure cardinality")
    seen_sources = set()
    for unit in witness.rows["units"]:
        source_id = struct.unpack_from("<I", unit, 4)[0]
        start, length = struct.unpack_from("<II", unit, 12)
        require(source_id < len(sources), "unit source ID")
        if start == 0xFFFF_FFFF:
            require(length == 0, "absent unit module span")
        else:
            require(re.fullmatch(rb"[A-Za-z_][A-Za-z0-9_]*",
                                 _span(sources[source_id], start, length)) is not None,
                    "unit module identity span")
        seen_sources.add(source_id)
    require(seen_sources == set(range(len(sources))), "complete source-unit custody")

    declarations: list[tuple[int, bytes]] = []
    for row in witness.rows["declarations"]:
        source_id, start, length = struct.unpack_from("<III", row, 8)[0], \
            struct.unpack_from("<I", row, 16)[0], struct.unpack_from("<I", row, 20)[0]
        require(source_id < len(sources), "declaration source ID")
        name = _span(sources[source_id], start, length)
        require(re.fullmatch(rb"[A-Za-z_][A-Za-z0-9_]*", name) is not None,
                "declaration authored identity")
        declarations.append((source_id, name))

    selected = witness.counts["selected"]
    require(selected < len(witness.rows["machines"]), "selected machine ID")
    machine = witness.rows["machines"][selected]
    declaration_id, record_id = struct.unpack_from("<II", machine, 4)
    require(declaration_id < len(declarations) and record_id < len(witness.rows["records"]),
            "selected machine ownership")
    record_declaration = struct.unpack_from("<I", witness.rows["records"][record_id], 4)[0]
    require(record_declaration < len(declarations), "selected owner declaration")
    root_owner = envelope.strings[envelope.root_owner_string_id].encode("ascii")
    root_machine = envelope.strings[envelope.root_machine_string_id].encode("ascii")
    require(declarations[declaration_id] == (envelope.root_source_id, root_machine),
            "selected machine equals OMGCOMP root")
    require(declarations[record_declaration] == (envelope.root_source_id, root_owner),
            "selected owner equals OMGCOMP root")
    return witness


def _arguments(raw: str) -> list[str]:
    return [part.strip() for part in raw.split(",") if part.strip()]


def parse_selected_source(omgcomp: bytes) -> SourceModel:
    envelope, sources = source_closure(omgcomp)
    source = sources[envelope.root_source_id]
    try:
        text = source.decode("ascii")
    except UnicodeDecodeError as error:
        raise RefinementError("selected source ASCII") from error
    text = re.sub(r"//[^\n]*|/\*.*?\*/", "", text, flags=re.S)
    signatures: dict[str, tuple[tuple[str, str], ...]] = {}
    state_positions: list[tuple[int, str]] = []
    signature = re.compile(r"(?:state\s+|machine\s+[A-Za-z_]\w*::)([A-Za-z_]\w*)\s*"
                           r"\(\s*&\s*(?:mut\s+)?self\s*((?:,[^)]*)?)\)")
    for match in signature.finditer(text):
        params = []
        for item in _arguments(match.group(2).lstrip(",")):
            require(":" in item, "parameter declaration")
            name, typ = item.split(":", 1)
            params.append((name.strip(), re.sub(r"\s+", "", typ)))
        signatures[match.group(1)] = tuple(params)
        if text[match.start():].startswith("state"):
            state_positions.append((match.start(), match.group(1)))

    guards: list[Guard] = []
    pattern = re.compile(r"transition\s+([A-Za-z_]\w*)\.len\s*>\s*0\s*\{\s*"
                         r"true\s*->\s*([A-Za-z_]\w*)\(([^)]*)\)\s*"
                         r"false\s*->\s*([A-Za-z_]\w*)\(([^)]*)\)\s*\}", re.S)
    for match in pattern.finditer(text):
        view, yes, yes_raw, no, no_raw = match.groups()
        enclosing = [name for position, name in state_positions if position < match.start()]
        require(bool(enclosing), "guard belongs to an authored state")
        current_params = dict(signatures[enclosing[-1]])
        require(current_params.get(view) == "&[u8]", "guarded expression is a direct &[u8] binder")
        yes_args, no_args = _arguments(yes_raw), _arguments(no_raw)
        head, tail = f"{view}[0]", f"{view}[1..]"
        require(yes_args.count(head) == yes_args.count(tail) == 1, "one direct head and tail argument")
        head_index, tail_index = yes_args.index(head), yes_args.index(tail)
        require(head_index < tail_index, "head precedes tail")
        passthrough = tuple(arg for arg in yes_args if arg not in (head, tail))
        require(tuple(no_args) == passthrough and len(set(passthrough)) == len(passthrough),
                "direct ordered pass-through binder vector")
        require(all(arg in current_params for arg in passthrough),
                "pass-through arguments are direct current-state binders")
        require(yes in signatures and no in signatures, "guard target declarations")
        yes_params, no_params = signatures[yes], signatures[no]
        require(len(yes_params) == len(yes_args) and len(no_params) == len(no_args),
                "guard target arity")
        require(yes_params[head_index][1] == "u8" and yes_params[tail_index][1] == "&[u8]",
                "head/tail target types")
        for (arg, (_, typ)), (_, no_typ) in zip(
                [(arg, yes_params[index]) for index, arg in enumerate(yes_args)
                 if index not in (head_index, tail_index)], no_params):
            require(arg in passthrough and current_params[arg] == typ == no_typ,
                    "pass-through exact type/order")
        guards.append(Guard(view, yes, no, head_index, tail_index, passthrough))
    require(len(guards) >= 2, "recurrent guarded-view source sites")

    literals = re.findall(r'"([\x20-\x21\x23-\x7e]*)"', text)
    require(len(literals) <= 1, "at most one unescaped selected byte literal")
    literal = literals[0].encode("ascii") if literals else None
    require(literal is None or len(literal) <= 32, "selected literal byte ceiling")
    # The selected source model deliberately owns only the small recurrent
    # accumulator shape: zero-initialized result, then one direct head write.
    assignments = re.findall(r"self\.result\s*=\s*([A-Za-z_]\w*)\s*;", text)
    require(len(assignments) == (1 if literal is not None else 0),
            "static accumulator assignment shape")
    if literal is not None:
        head_name = signatures[guards[0].true_target][guards[0].head_index][0]
        require(assignments[0] == head_name, "result receives direct head binder")
    initial = re.findall(r"self\.result\s*=\s*([0-9]+)\s*;", text)
    require(len(initial) <= 1 and (not initial or 0 <= int(initial[0]) <= 255),
            "optional u8 result initialization")
    if literal is None:
        entry = text[:text.find("state ")]
        direct = re.findall(r"(?:\{|;)\s*([0-9]+)\s*(?=state|$)", entry)
        require(len(direct) == 1 and 0 <= int(direct[0]) <= 255,
                "runtime-origin direct source result")
        result = int(direct[0])
    else:
        result = literal[-1] if literal else (int(initial[0]) if initial else 0)
    return SourceModel(source, tuple(guards), literal, result)
