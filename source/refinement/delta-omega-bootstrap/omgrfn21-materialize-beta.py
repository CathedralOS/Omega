#!/usr/bin/env python3
"""Materialize representative persisted-Beta projections for OMGRFN21."""

from __future__ import annotations

import argparse
from pathlib import Path

from omgrfn21_ckir import IR18, arguments, decode
from omgrfn21_frame import HEADER
from omgrfn21_profiles import canonical
from omgrfn21_source import ROWS as WROWS
from omgrfn21_source import decode_witness, source_closure


def extents(frame: bytes) -> dict[str, tuple[int, int]]:
    words = HEADER.unpack_from(frame)
    at = HEADER.size
    result = {"header": (0, HEADER.size)}
    for name, length in zip(("omgcomp", "witness", "ckir", "elf"), words[3:7]):
        result[name] = (at, length); at += length
    return result


def context():
    frame = canonical()
    spans = extents(frame)
    omg_at, _ = spans["omgcomp"]
    witness_at, witness_len = spans["witness"]
    ckir_at, _ = spans["ckir"]
    elf_at, _ = spans["elf"]
    witness = decode_witness(frame[witness_at:witness_at + witness_len])
    _, source = source_closure(frame[omg_at:omg_at + spans["omgcomp"][1]])
    source_at = frame.find(source, omg_at, witness_at)
    if source_at < 0 or frame.find(source, source_at + 1, witness_at) >= 0:
        raise RuntimeError("unique authored source extent")
    ckir = decode(frame[ckir_at:elf_at])
    offsets = {}
    cursor = ckir_at + IR18.HEADER.size
    for name in IR18.TABLE_ORDER:
        extent = len(ckir.tables[name]) * IR18.ROWS[name].size
        offsets[name] = (cursor, extent); cursor += extent
    return frame, spans, witness, source_at, ckir, offsets


def wrow(witness, witness_at: int, name: str, row: int) -> tuple[int, int]:
    start = witness_at + witness.offsets[name][0]
    return start + row * WROWS[name].size, WROWS[name].size


def crow(offsets, name: str, row: int) -> tuple[int, int]:
    start, _ = offsets[name]
    return start + row * IR18.ROWS[name].size, IR18.ROWS[name].size


def selected_operation_ids(module) -> set[int]:
    selected = set()
    for operation in module.tables["operations"]:
        args = arguments(module, operation)
        if operation[3] in (4, 8, 9) and any(
                value < len(module.value_types)
                and module.tables["types"][module.value_types[value]][1] == 8
                for value in args):
            selected.add(operation[0])
    # Include direct producers/consumers of selected operands/results.
    selected_values = {module.tables["operations"][op][6] for op in selected
                       if module.tables["operations"][op][6] != IR18.NO_ID}
    changed = True
    while changed:
        changed = False
        for operation in module.tables["operations"]:
            args = set(arguments(module, operation))
            if ((operation[6] != IR18.NO_ID and operation[6] in selected_values)
                    or args & selected_values) \
                    and operation[0] not in selected:
                selected.add(operation[0])
                if operation[6] != IR18.NO_ID:
                    selected_values.add(operation[6])
                changed = True
    return selected


def owned(ctx, owner: str) -> tuple[tuple[int, int], ...]:
    frame, spans, witness, source_at, module, offsets = ctx
    witness_at = spans["witness"][0]
    ckir_at = spans["ckir"][0]
    selected = sorted(selected_operation_ids(module))
    result = [(0, HEADER.size)]
    if owner == "r1":
        result += [(spans[name][0], 64 if name != "witness" else 128)
                   for name in ("omgcomp", "witness", "ckir", "elf")]
    elif owner == "r2":
        result.append((witness_at, 128))
        result.append((witness_at + witness.offsets["types"][0],
                       witness.offsets["types"][1]))
        result.append(wrow(witness, witness_at, "records", witness.selected_record))
        selected_record = witness.tables["records"][witness.selected_record]
        for row in range(selected_record[3], selected_record[3] + selected_record[4]):
            result.append(wrow(witness, witness_at, "fields", row))
        for mid in (witness.clear_machine, witness.append_machine,
                    witness.lookup_machine, witness.selected_root):
            result.append(wrow(witness, witness_at, "machines", mid))
        for row in range(len(witness.tables["params"])):
            result.append(wrow(witness, witness_at, "params", row))
        for row in range(len(witness.tables["blocks"])):
            result.append(wrow(witness, witness_at, "blocks", row))
        for row in (0, 1, 2):
            result.append(wrow(witness, witness_at, "calls", row))
        for mid in (witness.append_machine, witness.lookup_machine):
            machine = witness.tables["machines"][mid]
            result.append((source_at + machine[11], machine[12]))
    elif owner == "r3":
        result.append((ckir_at, IR18.HEADER.size))
        for name in ("types", "records", "fields", "machines", "machine_params", "blocks"):
            result.append(offsets[name])
        result += [crow(offsets, "operations", op) for op in selected]
        for op in selected:
            operation = module.tables["operations"][op]
            if operation[9]:
                result.append((offsets["operands"][0] + 4 * operation[8],
                               4 * operation[9]))
        result += [crow(offsets, "terminators", row) for row in (1, 4)]
    elif owner == "r4":
        for mid in (witness.append_machine, witness.lookup_machine):
            machine = witness.tables["machines"][mid]
            result.append((source_at + machine[11], machine[12]))
            result.append(wrow(witness, witness_at, "machines", mid))
        for row in (witness.u8_type, witness.bool_type, witness.index_type,
                    witness.length_type, witness.array_type):
            result.append(wrow(witness, witness_at, "types", row))
        selected_record = witness.tables["records"][witness.selected_record]
        for row in range(selected_record[3], selected_record[3] + selected_record[4]):
            result.append(wrow(witness, witness_at, "fields", row))
        for row in range(len(witness.tables["params"])):
            result.append(wrow(witness, witness_at, "params", row))
        for row in range(1, 7):
            result.append(wrow(witness, witness_at, "blocks", row))
        result += [crow(offsets, "operations", op) for op in selected]
        result += [crow(offsets, "terminators", row) for row in (1, 4)]
    elif owner == "r5":
        result.append((ckir_at, IR18.HEADER.size))
        for name in ("types", "machines", "machine_params", "blocks"):
            result.append(offsets[name])
        trace_ops = sorted(set(selected) | {43, 44, 58})
        result += [crow(offsets, "operations", op) for op in trace_ops]
        for op in trace_ops:
            operation = module.tables["operations"][op]
            if operation[9]:
                result.append((offsets["operands"][0] + 4 * operation[8],
                               4 * operation[9]))
        result += [crow(offsets, "terminators", row) for row in (1, 2, 4, 5, 7)]
        elf_at, elf_len = spans["elf"]
        result.append((elf_at, 64))
        elf = frame[elf_at:elf_at + elf_len]
        for needle in (b"\x49\xb9", b"\x0f\x83", b"\x48\x03\x85",
                       b"\x0f\x82", b"\x48\x3b\x85"):
            at = elf.find(needle)
            if at < 0:
                raise RuntimeError(f"missing ELF projection {needle!r}")
            result.append((elf_at + max(0, at - 8), min(32, elf_len - max(0, at - 8))))
    else:
        raise RuntimeError(owner)
    return tuple(result)


def segments(frame: bytes, spans):
    for start, length in spans:
        end, cursor = start + length, start
        literals = []
        while cursor < end:
            run = 1
            while cursor + run < end and frame[cursor + run] == frame[cursor]:
                run += 1
            if run >= 8:
                if literals:
                    yield "literal", tuple(literals); literals = []
                yield "run", cursor, cursor + run, frame[cursor]; cursor += run
            else:
                literals.append((cursor, frame[cursor])); cursor += 1
                if len(literals) == 20:
                    yield "literal", tuple(literals); literals = []
        if literals:
            yield "literal", tuple(literals)


def checker(frame: bytes, spans) -> str:
    states = []
    pieces = list(segments(frame, spans))
    for index, piece in enumerate(pieces):
        nxt = f"s{index + 1}" if index + 1 < len(pieces) else "accepted"
        if piece[0] == "run":
            _, start, end, value = piece
            states.append(f"state s{index} {{ i={start} to run{index} }}\n"
                          f"state run{index} {{ to {nxt} when(i>={end}) "
                          f"to bad when(fb(i)!={value}) i=i+1 to run{index} }}")
        else:
            checks = " ".join(f"to bad when(fb({at})!={value})" for at, value in piece[1])
            states.append(f"state s{index} {{ {checks} to {nxt} }}")
    return f'''proc fb(index) {{ return byte[1048576+index] }}
proc read_frame() {{
 let n=0 let c=read_byte()
 state read {{ to one when(c>=0) to finish }}
 state one {{ to bad when(n>={len(frame)}) byte[1048576+n]=c n=n+1 c=read_byte() to read }}
 state finish {{ to bad when(n!={len(frame)}) return 0 }}
 state bad {{ return 251 }}
}}
proc check_owned() {{
 let i=0
 state begin {{ to s0 }}
 {chr(10).join(states)}
 state accepted {{ return 0 }}
 state bad {{ return 251 }}
}}
proc main() {{
 let status=read_frame()
 state begin {{ to done when(status!=0) status=check_owned() to done }}
 state done {{ return status }}
}}
'''


def materialize(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    ctx = context(); frame = ctx[0]
    (output / "canonical.rfn").write_bytes(frame)
    manifest = []
    for owner in ("r1", "r2", "r3", "r4", "r5"):
        spans = owned(ctx, owner)
        (output / f"{owner}.beta").write_text(checker(frame, spans), encoding="ascii")
        reject_at = spans[-1][0]
        reject = bytearray(frame); reject[reject_at] ^= 1
        (output / f"{owner}-reject.rfn").write_bytes(reject)
        manifest.append(f"{owner}\t{reject_at}\n")
    (output / "manifest.tsv").write_text("".join(manifest), encoding="ascii")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(); parser.add_argument("output", type=Path)
    materialize(parser.parse_args().output)
