#!/usr/bin/env python3
"""Handcrafted CKIR20 full TokenStream::push carrier and mutation corpus."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import struct
from pathlib import Path

import checked_ir_v20_reference as ir20


NO_ID = ir20.NO_ID


def replace(row: tuple[int, ...], index: int, value: int) -> tuple[int, ...]:
    return row[:index] + (value,) + row[index + 1:]


class Builder:
    def __init__(self, initial_values: int) -> None:
        self.operations: list[tuple[int, ...]] = []
        self.operands: list[tuple[int]] = []
        self.blocks: list[tuple[int, ...]] = []
        self.values = initial_values
        self.places = 0
        self.terms: dict[int, dict[str, object]] = {}

    def operation(self, owner: int, block: int, opcode: int,
                  result_type: int | None, args: tuple[int, ...] = (),
                  *, place: bool = False, imm0: int = 0, imm1: int = 0) -> int:
        op_id = len(self.operations)
        start = len(self.operands)
        self.operands.extend((value,) for value in args)
        if result_type is None:
            result_kind, result_id, wire_type, returned = 0, NO_ID, NO_ID, NO_ID
        elif place:
            result_kind, result_id, wire_type = 2, self.places, result_type
            returned = self.places
            self.places += 1
        else:
            result_kind, result_id, wire_type = 1, self.values, result_type
            returned = self.values
            self.values += 1
        self.operations.append((
            op_id, owner, block, opcode, result_kind, 0, result_id, wire_type,
            start, len(args), imm0, imm1,
        ))
        return returned

    def block(self, owner: int, access: int, param_start: int,
              param_count: int, body) -> int:
        block_id = len(self.blocks)
        start = len(self.operations)
        body(block_id)
        self.blocks.append((
            block_id, owner, access, 0, 0, param_start, param_count,
            start, len(self.operations) - start, block_id,
        ))
        return block_id

    def term(self, block: int, kind: int, *, value: int = NO_ID,
             target0: int = NO_ID, args0: tuple[int, ...] = (),
             target1: int = NO_ID, args1: tuple[int, ...] = (),
             flags: int = 0, arm_start: int = 0, arm_count: int = 0) -> None:
        self.terms[block] = {
            "kind": kind, "value": value, "target0": target0, "args0": args0,
            "target1": target1, "args1": args1, "flags": flags,
            "arm_start": arm_start, "arm_count": arm_count,
        }


def declarations(array_tokens: int = 16_384, decoded_bytes: int = 65_536):
    types = [
        (0, 4, 0, 0, 0, 0, 0, 0),   # SourceId
        (1, 4, 0, 0, 1, 0, 0, 0),   # Span
        (2, 4, 0, 0, 2, 0, 0, 0),   # SourceSpan
        (3, 4, 0, 0, 3, 0, 0, 0),   # Token
        (4, 4, 0, 0, 4, 0, 0, 0),   # LexDiagnostic
        (5, 4, 0, 0, 5, 0, 0, 0),   # TokenObservation
        (6, 4, 0, 0, 6, 0, 0, 0),   # TokenStream
        (7, 4, 0, 0, 7, 0, 0, 0),   # Main
        (8, 6, 0, 0, 0, 0, 0, 0),   # NumericBase
        (9, 6, 0, 0, 1, 0, 0, 0),   # KeywordKind
        (10, 6, 0, 0, 2, 0, 0, 0),  # PunctuationKind
        (11, 6, 0, 0, 3, 0, 0, 0),  # TokenKind
        (12, 6, 0, 0, 4, 0, 0, 0),  # LexDiagnosticCode
        (13, 1, 0, 0, 0, 0, 0, 255),
        (14, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),
        (15, 3, 0, 0, 0, 0, 0, 1),
        (16, 8, 0, 0, 0, 0, 0xFFFF_FFFF, 0xFFFF_FFFF),
        (17, 8, 0, 0, 0, 0, 16_384, 0),
        (18, 8, 0, 0, 0, 0, 0, 0),
        (19, 8, 0, 0, 1, 0, 1, 0),
        (20, 8, 0, 0, 16_384, 0, 16_384, 0),
        (21, 5, 1, 0, 3, array_tokens, 0, 0),
        (22, 5, 1, 0, 5, 16_384, 0, 0),
        (23, 5, 1, 0, 13, decoded_bytes, 0, 0),
    ]
    record_field_types = [
        [14], [16, 16], [0, 1], [11, 2, 16, 16], [12, 2],
        [13, 13, 13, 13, 14, 16, 16, 16, 16],
        [21, 22, 17, 23, 16, 4, 15, 15], [6],
    ]
    fields: list[tuple[int, ...]] = []
    records: list[tuple[int, ...]] = []
    for record_id, field_types in enumerate(record_field_types):
        start = len(fields)
        fields.extend((len(fields), record_id, ordinal, type_id)
                      for ordinal, type_id in enumerate(field_types))
        records.append((record_id, record_id, start, len(field_types),
                        int(record_id <= 5), 0, 0, 0))

    case_counts = [4, 30, 42, 9, 20]
    sums: list[tuple[int, ...]] = []
    cases: list[tuple[int, ...]] = []
    payloads: list[tuple[int, ...]] = []
    token_payload_types = {
        1: [8, 15, 15], 2: [15, 15, 15], 4: [9], 5: [10],
    }
    for sum_id, count in enumerate(case_counts):
        start = len(cases)
        for ordinal in range(count):
            selected = token_payload_types.get(ordinal, []) if sum_id == 3 else []
            payload_start = len(payloads)
            case_id = len(cases)
            payloads.extend((len(payloads), case_id, payload_ordinal, type_id)
                            for payload_ordinal, type_id in enumerate(selected))
            cases.append((case_id, sum_id, ordinal, payload_start, len(selected)))
        sums.append((sum_id, 8 + sum_id, start, count, 1, 0, 0, 0))
    return types, records, fields, sums, cases, payloads


def tables(*, extra: str = "none", array_tokens: int = 16_384,
           decoded_bytes: int = 65_536) -> dict[str, list[tuple[int, ...]]]:
    t = {name: [] for name in ir20.TABLE_ORDER}
    (t["types"], t["records"], t["fields"], t["sums"],
     t["cases"], t["case_payloads"]) = declarations(array_tokens, decoded_bytes)
    t["machines"] = [
        (0, 6, 2, 0, 0, NO_ID, 0, 10, 0, 3, 0),
        (1, 6, 1, 0, 0, 13, 10, 1, 3, 10, 3),
        (2, 7, 2, 0, 0, 13, 11, 0, 13, 1, 13),
    ]
    param_types = [0, 11, 16, 16, 16, 16, 13, 13, 13, 13, 16]
    t["machine_params"] = [
        (pid, 0 if pid < 10 else 1, pid if pid < 10 else 0, type_id, pid)
        for pid, type_id in enumerate(param_types)
    ]
    block_param_types = [8, 15, 15, 15, 15, 15, 9, 10, 15, 15, 15]
    block_param_owners = [5, 5, 5, 6, 6, 6, 7, 8, 9, 9, 10]
    t["block_params"] = [
        (pid, block_param_owners[pid],
         sum(1 for owner in block_param_owners[:pid] if owner == block_param_owners[pid]),
         type_id, 11 + pid)
        for pid, type_id in enumerate(block_param_types)
    ]

    b = Builder(initial_values=22)

    def self_field(owner: int, block: int, self_type: int,
                   field_id: int, field_type: int) -> int:
        self_p = b.operation(owner, block, 2, self_type, place=True)
        return b.operation(owner, block, 3, field_type, (self_p,),
                           place=True, imm0=field_id)

    def indexed(owner: int, block: int, array_field: int, array_type: int,
                element_type: int, index_value: int) -> int:
        self_p = b.operation(owner, block, 2, 6, place=True)
        array_p = b.operation(owner, block, 3, array_type, (self_p,),
                              place=True, imm0=array_field)
        return b.operation(owner, block, 4, element_type,
                           (array_p, index_value), place=True)

    def current_index(owner: int, block: int) -> int:
        count_p = self_field(owner, block, 6, 22, 17)
        return b.operation(owner, block, 5, 17, (count_p,))

    def token_place(block: int) -> int:
        return indexed(0, block, 20, 21, 3, current_index(0, block))

    def observation_place(block: int) -> int:
        return indexed(0, block, 21, 22, 5, current_index(0, block))

    def push_entry(block: int) -> None:
        retained = self_field(0, block, 6, 27, 15)
        false = b.operation(0, block, 1, 15)
        b.operation(0, block, 6, None, (retained, false))
        count = current_index(0, block)
        capacity = b.operation(0, block, 1, 20, imm0=16_384)
        condition = b.operation(0, block, 9, 15, (count, capacity))
        b.term(block, 2, value=condition, target0=1, target1=2)
    b.block(0, 2, 0, 0, push_entry)

    def push_retain(block: int) -> None:
        token = token_place(block)
        kind_p = b.operation(0, block, 3, 11, (token,), place=True, imm0=5)
        b.operation(0, block, 7, None, (kind_p, 1), imm0=1)

        token = token_place(block)
        source_span = b.operation(0, block, 3, 2, (token,), place=True, imm0=6)
        source_id = b.operation(0, block, 3, 0, (source_span,), place=True, imm0=3)
        b.operation(0, block, 7, None, (source_id, 0), imm0=1)
        source_value_p = b.operation(0, block, 3, 14, (source_id,), place=True, imm0=0)
        source_value = b.operation(0, block, 5, 14, (source_value_p,))

        for field_id, parameter in ((1, 2), (2, 3)):
            token = token_place(block)
            source_span = b.operation(0, block, 3, 2, (token,), place=True, imm0=6)
            span_p = b.operation(0, block, 3, 1, (source_span,), place=True, imm0=4)
            scalar_p = b.operation(0, block, 3, 16, (span_p,), place=True,
                                   imm0=field_id)
            b.operation(0, block, 6, None, (scalar_p, parameter))
        for field_id, parameter in ((7, 4), (8, 5)):
            token = token_place(block)
            scalar_p = b.operation(0, block, 3, 16, (token,), place=True,
                                   imm0=field_id)
            b.operation(0, block, 6, None, (scalar_p, parameter))

        observation_values = [6, 7, 8, 9, source_value, 2, 3, 4, 5]
        for field_id, value in zip(range(11, 20), observation_values):
            observation = observation_place(block)
            field_type = t["fields"][field_id][3]
            field_p = b.operation(0, block, 3, field_type, (observation,),
                                  place=True, imm0=field_id)
            b.operation(0, block, 6, None, (field_p, value))

        count_p = self_field(0, block, 6, 22, 17)
        count = b.operation(0, block, 5, 17, (count_p,))
        one = b.operation(0, block, 1, 19, imm0=1)
        incremented = b.operation(0, block, 8, 17, (count, one))
        b.operation(0, block, 6, None, (count_p, incremented))
        retained = self_field(0, block, 6, 27, 15)
        true = b.operation(0, block, 1, 15, imm0=1)
        b.operation(0, block, 6, None, (retained, true))
        if extra == "copy-alias":
            token = token_place(block)
            kind_p = b.operation(0, block, 3, 11, (token,), place=True, imm0=5)
            b.operation(0, block, 7, None, (kind_p, kind_p), imm0=2)
        b.term(block, 3)
    b.block(0, 2, 0, 0, push_retain)

    def push_full(block: int) -> None:
        retained = self_field(0, block, 6, 27, 15)
        false = b.operation(0, block, 1, 15)
        b.operation(0, block, 6, None, (retained, false))
        b.term(block, 3)
    b.block(0, 2, 0, 0, push_full)

    def read_entry(block: int) -> None:
        count_p = self_field(1, block, 6, 22, 17)
        count = b.operation(1, block, 5, 17, (count_p,))
        condition = b.operation(1, block, 9, 15, (10, count))
        b.term(block, 2, value=condition, target0=4, target1=12)
    b.block(1, 1, 0, 0, read_entry)

    def read_present(block: int) -> None:
        token = indexed(1, block, 20, 21, 3, 10)
        kind_p = b.operation(1, block, 3, 11, (token,), place=True, imm0=5)
        b.term(block, 5, value=kind_p, flags=2, arm_start=0, arm_count=9)
    b.block(1, 1, 0, 0, read_present)

    def integer_drop(block: int) -> None:
        b.term(block, 1, target0=12)
    b.block(1, 1, 0, 3, integer_drop)

    def exponent(block: int) -> None:
        b.term(block, 2, value=14, target0=9, args0=(15, 16), target1=12)
    b.block(1, 1, 3, 3, exponent)

    def keyword_drop(block: int) -> None:
        b.term(block, 1, target0=12)
    b.block(1, 1, 6, 1, keyword_drop)

    def punctuation_drop(block: int) -> None:
        b.term(block, 1, target0=12)
    b.block(1, 1, 7, 1, punctuation_drop)

    def empty(block: int) -> None:
        b.term(block, 2, value=19, target0=12, target1=10, args1=(20,))
    b.block(1, 1, 8, 2, empty)

    def suffix(block: int) -> None:
        b.term(block, 2, value=21, target0=11, target1=12)
    b.block(1, 1, 10, 1, suffix)

    def retained_tag(block: int) -> None:
        observation = indexed(1, block, 21, 22, 5, 10)
        tag_p = b.operation(1, block, 3, 13, (observation,), place=True, imm0=11)
        tag = b.operation(1, block, 5, 13, (tag_p,))
        b.term(block, 4, value=tag)
    b.block(1, 1, 11, 0, retained_tag)

    def absent(block: int) -> None:
        zero = b.operation(1, block, 1, 13)
        b.term(block, 4, value=zero)
    b.block(1, 1, 11, 0, absent)

    def run(block: int) -> None:
        self_p = b.operation(2, block, 2, 7, place=True)
        stream_p = b.operation(2, block, 3, 6, (self_p,), place=True, imm0=28)
        if extra == "full-path":
            count_p = b.operation(2, block, 3, 17, (stream_p,), place=True, imm0=22)
            capacity = b.operation(2, block, 1, 20, imm0=16_384)
            b.operation(2, block, 6, None, (count_p, capacity))
        source_scalar = b.operation(2, block, 1, 14, imm0=4)
        source = b.operation(2, block, 13, 0, (source_scalar,))
        true0 = b.operation(2, block, 1, 15, imm0=1)
        false = b.operation(2, block, 1, 15)
        true1 = b.operation(2, block, 1, 15, imm0=1)
        kind = b.operation(2, block, 14, 11, (true0, false, true1), imm0=78)
        high = extra == "high-half-transport"
        u64_values = [
            b.operation(2, block, 1, 16, imm0=value,
                        imm1=(ordinal + 1 if high else 0))
            for ordinal, value in enumerate((5, 6, 7, 8))
        ]
        byte_values = [b.operation(2, block, 1, 13, imm0=value)
                       for value in (70, 1, 2, 3)]
        b.operation(2, block, 10, None,
                    (stream_p, source, kind, *u64_values, *byte_values), imm0=0)
        index = b.operation(2, block, 1, 16)
        result = b.operation(2, block, 10, 13, (stream_p, index), imm0=1)
        if extra in ("index-oob-high", "index-oob-bound"):
            tokens_p = b.operation(2, block, 3, 21, (stream_p,), place=True, imm0=20)
            bad_index = (b.operation(2, block, 1, 16, imm1=1)
                         if extra == "index-oob-high" else
                         b.operation(2, block, 1, 16, imm0=16_384))
            bad_token = b.operation(2, block, 4, 3, (tokens_p, bad_index), place=True)
            kind_p = b.operation(2, block, 3, 11, (bad_token,), place=True, imm0=5)
            b.operation(2, block, 7, None, (kind_p, kind_p), imm0=2)
        b.term(block, 4, value=result)
    b.block(2, 2, 11, 0, run)

    t["blocks"] = b.blocks
    t["operations"] = b.operations
    t["case_arms"] = []
    t["case_arm_args"] = []
    target_by_ordinal = [12, 5, 6, 12, 7, 8, 12, 12, 12]
    payload_by_ordinal = [(), (0, 1, 2), (3, 4, 5), (), (6,), (7,), (), (), ()]
    for ordinal in range(9):
        start = len(t["case_arm_args"])
        for payload_id in payload_by_ordinal[ordinal]:
            t["case_arm_args"].append((len(t["case_arm_args"]), 2, payload_id))
        t["case_arms"].append((ordinal, 4, 76 + ordinal,
                               target_by_ordinal[ordinal], start,
                               len(payload_by_ordinal[ordinal])))

    t["terminators"] = []
    arm_cursor = 0
    for block_id, block in enumerate(b.blocks):
        term = b.terms[block_id]
        start0 = len(b.operands)
        b.operands.extend((value,) for value in term["args0"])
        start1 = len(b.operands)
        b.operands.extend((value,) for value in term["args1"])
        arm_start = int(term["arm_start"]) if term["kind"] == 5 else arm_cursor
        if term["kind"] == 5:
            arm_cursor += int(term["arm_count"])
        t["terminators"].append((
            block_id, block[1], block_id, term["kind"], term["flags"], 0,
            term["value"], term["target0"], start0, len(term["args0"]),
            term["target1"], start1, len(term["args1"]),
            arm_start, term["arm_count"],
        ))
    t["operands"] = b.operands
    t["_counts"] = [(b.values, b.places)]
    return t


def encode(raw: dict[str, list[tuple[int, ...]]], *, major: int = 20,
           entry: int = 2) -> bytes:
    values, places = raw["_counts"][0]
    counts = {name: len(raw[name]) for name in ir20.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(ir20.ROWS[name].pack(*row)
                       for name in ir20.TABLE_ORDER for row in raw[name])
    return ir20.HEADER.pack(
        b"OMGCKIR\0", major, 0, 1, int(entry != NO_ID), entry,
        ir20.HEADER.size + len(payload),
        *(counts[name] for name in ir20.COUNT_NAMES),
    ) + payload


def malformed() -> dict[str, tuple[bytes, int]]:
    base = tables()
    result: dict[str, tuple[bytes, int]] = {}

    def one(name: str, change, status: int = 251) -> None:
        changed = copy.deepcopy(base)
        change(changed)
        result[name] = (encode(changed), status)

    result["major-19"] = (encode(base, major=19), 251)
    result["absent-entry"] = (encode(base, entry=NO_ID), 251)
    result["trailing-byte"] = (encode(base) + b"\0", 251)
    one("token-noncopy", lambda t: t["records"].__setitem__(
        3, replace(t["records"][3], 4, 0)))
    one("tokenkind-noncopy", lambda t: t["sums"].__setitem__(
        3, replace(t["sums"][3], 4, 0)))
    one("float-payload-type", lambda t: t["case_payloads"].__setitem__(
        3, replace(t["case_payloads"][3], 3, 13)))
    one("missing-kind-copy", lambda t: t["operations"].__setitem__(
        next(i for i, op in enumerate(t["operations"]) if op[3] == 7),
        replace(next(op for op in t["operations"] if op[3] == 7), 3, 6)))
    copy_indices = [i for i, op in enumerate(base["operations"]) if op[3] == 7]
    one("copy-wrong-source", lambda t: t["operands"].__setitem__(
        t["operations"][copy_indices[0]][8] + 1, (0,)))
    one("wrong-observation-store", lambda t: t["operations"].__setitem__(
        next(i for i, op in enumerate(t["operations"])
             if op[3] == 3 and op[10] == 19),
        replace(next(op for op in t["operations"]
                     if op[3] == 3 and op[10] == 19), 10, 18)))
    one("wrong-call-arity", lambda t: t["operations"].__setitem__(
        next(i for i, op in enumerate(t["operations"])
             if op[3] == 10 and op[10] == 0),
        replace(next(op for op in t["operations"]
                     if op[3] == 10 and op[10] == 0), 9, 10)))
    one("wrong-float-arm", lambda t: t["case_arms"].__setitem__(
        2, replace(t["case_arms"][2], 2, 77)))
    one("forbidden-opcode", lambda t: t["operations"].__setitem__(
        0, replace(t["operations"][0], 3, 15)))
    one("u64-policy-flag", lambda t: t["types"].__setitem__(
        16, replace(t["types"][16], 2, 1)))
    one("owner-over-2m", lambda t: (
        t["types"].__setitem__(21, replace(t["types"][21], 5, 25_000))), 252)
    one("array-65537", lambda t: t["types"].__setitem__(
        23, replace(t["types"][23], 5, 65_537)))

    raw = bytearray(encode(base))
    struct.pack_into("<I", raw, 24 + 4 * ir20.COUNT_NAMES.index("operations"), 65_537)
    result["operation-exhaustion"] = (bytes(raw), 252)
    return result


def emit(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    positives = [("canonical", "none", 70), ("high-half-transport", "high-half-transport", 70),
                 ("copy-alias", "copy-alias", 70), ("full-path", "full-path", 0)]
    for name, extra, outcome in positives:
        contents = encode(tables(extra=extra))
        (directory / f"{name}.ckir20").write_bytes(contents)
    (directory / "positives.tsv").write_text(
        "".join(f"{name}\t{outcome}\n" for name, _, outcome in positives), encoding="ascii")
    runtime = ["index-oob-high", "index-oob-bound"]
    for name in runtime:
        (directory / f"{name}.ckir20").write_bytes(encode(tables(extra=name)))
    (directory / "runtime.tsv").write_text("".join(f"{name}\n" for name in runtime),
                                            encoding="ascii")
    failures = malformed()
    for name, (contents, _) in failures.items():
        (directory / f"{name}.ckir20").write_bytes(contents)
    (directory / "manifest.tsv").write_text(
        "".join(f"{name}\t{status}\n" for name, (_, status) in failures.items()),
        encoding="ascii")
    canonical = (directory / "canonical.ckir20").read_bytes()
    (directory / "identity.json").write_text(json.dumps({
        "bytes": len(canonical), "sha256": hashlib.sha256(canonical).hexdigest(),
    }, sort_keys=True) + "\n", encoding="ascii")


def check(path: Path, outcome: int) -> None:
    module = ir20.decode(path.read_bytes())
    ir20.v5.require(ir20.interpret(module) == outcome, "CKIR20 expected outcome")
    if path.name == "canonical.ckir20":
        ir20.v5.require(module.layouts[7] == (1_638_456, 8), "canonical owner layout")
        corrupted = copy.deepcopy(module)
        # The constructor writes TokenKind::Float (ordinal 2). Shrinking the
        # selected sum's runtime domain after validation makes that stored tag
        # invalid and must be caught by CaseDispatch before payload access.
        corrupted.tables["sums"][3] = replace(corrupted.tables["sums"][3], 3, 2)
        try:
            ir20.interpret(corrupted)
        except ir20.Ckir20Error:
            pass
        else:
            raise ir20.Ckir20Error("runtime invalid sum tag accepted")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("emit", "check", "identity"))
    parser.add_argument("path", type=Path)
    parser.add_argument("outcome", nargs="?", type=int)
    args = parser.parse_args()
    if args.command == "emit":
        emit(args.path)
    elif args.command == "check":
        check(args.path, int(args.outcome))
    else:
        raw = encode(tables())
        print(len(raw), hashlib.sha256(raw).hexdigest())


if __name__ == "__main__":
    main()
