#!/usr/bin/env python3
"""Materialize split representative persisted-Beta projections for OMGRFN23."""

from __future__ import annotations

import argparse
from pathlib import Path

from omgrfn23_ckir import IR20, producer_decode
from omgrfn23_frame import HEADER
from omgrfn23_profiles import canonical
from omgrfn23_source import ROWS as WROWS
from omgrfn23_source import decode_witness, source_closure


def merge(spans):
    merged: list[list[int]] = []
    for start, length in sorted(set(spans)):
        if length <= 0:
            continue
        end = start + length
        if merged and start <= merged[-1][1]:
            merged[-1][1] = max(merged[-1][1], end)
        else:
            merged.append([start, end])
    return tuple((start, end - start) for start, end in merged)


def chunks(start: int, length: int, count: int):
    cuts = [length * index // count for index in range(count + 1)]
    return [(start + cuts[index], cuts[index + 1] - cuts[index])
            for index in range(count)]


def context():
    frame = canonical(); words = HEADER.unpack_from(frame)
    cursor = HEADER.size; extents = {"header": (0, HEADER.size)}
    for name, length in zip(("omgcomp", "witness", "ckir", "elf"), words[3:7]):
        extents[name] = (cursor, length); cursor += length
    omg_at, omg_len = extents["omgcomp"]
    wit_at, wit_len = extents["witness"]
    ckir_at, ckir_len = extents["ckir"]
    witness = decode_witness(frame[wit_at:wit_at + wit_len])
    _, source = source_closure(frame[omg_at:omg_at + omg_len])
    source_at = frame.find(source, omg_at, wit_at)
    if source_at < 0 or frame.find(source, source_at + 1, wit_at) >= 0:
        raise RuntimeError("unique authored source extent")
    module = producer_decode(frame[ckir_at:ckir_at + ckir_len])
    offsets = {}; cursor = ckir_at + IR20.HEADER.size
    for name in IR20.TABLE_ORDER:
        extent = len(module.tables[name]) * IR20.ROWS[name].size
        offsets[name] = (cursor, extent); cursor += extent
    return frame, extents, witness, source_at, len(source), module, offsets


def table_span(witness, base: int, name: str):
    return base + witness.offsets[name][0], witness.offsets[name][1]


def projections(ctx):
    frame, ext, witness, source_at, source_len, module, coff = ctx
    wit_at = ext["witness"][0]; ckir_at = ext["ckir"][0]
    result: dict[str, list[tuple[int, int]]] = {
        "r1": [(0, HEADER.size)] +
              [(ext[name][0], min(ext[name][1], 192))
               for name in ("omgcomp", "witness", "ckir", "elf")],
        "r2-header": [(0, HEADER.size), (wit_at, 192)],
    }
    for name in ("types", "records", "fields", "sums", "payloads",
                 "machines", "params", "blocks", "block_params", "calls",
                 "stores", "store_paths", "arguments"):
        result[f"r2-{name}"] = [(0, HEADER.size), table_span(witness, wit_at, name)]
    case_at, case_len = table_span(witness, wit_at, "cases")
    for index, span in enumerate(chunks(case_at, case_len, 6)):
        result[f"r2-cases-{index}"] = [(0, HEADER.size), span]
    for index, span in enumerate(chunks(source_at, source_len, 8)):
        result[f"r2-source-{index}"] = [(0, HEADER.size), span]

    result["r3-header"] = [(0, HEADER.size), (ckir_at, IR20.HEADER.size)]
    for group, names in enumerate((
        ("types", "records", "fields"), ("sums", "case_payloads"),
        ("machines", "machine_params", "blocks", "block_params"),
        ("terminators", "case_arms", "case_arm_args"),
    )):
        result[f"r3-decls-{group}"] = [(0, HEADER.size)] + [coff[name] for name in names]
    case_at, case_len = coff["cases"]
    for index, span in enumerate(chunks(case_at, case_len, 6)):
        result[f"r3-cases-{index}"] = [(0, HEADER.size), span]
    op_at, op_len = coff["operations"]
    operand_at, operand_len = coff["operands"]
    op_chunks = chunks(op_at, op_len, 10); operand_chunks = chunks(operand_at, operand_len, 10)
    for index in range(10):
        result[f"r3-ops-{index}"] = [(0, HEADER.size), op_chunks[index],
                                     operand_chunks[index]]

    stores = table_span(witness, wit_at, "stores")
    paths = table_span(witness, wit_at, "store_paths")
    r4_count = 12
    r4_ops = chunks(op_at, op_len, r4_count)
    r4_operands = chunks(operand_at, operand_len, r4_count)
    for index, source_piece in enumerate(chunks(source_at, source_len, r4_count)):
        result[f"r4-{index}"] = [(0, HEADER.size), source_piece,
                                 chunks(*stores, r4_count)[index],
                                 chunks(*paths, r4_count)[index],
                                 r4_ops[index], r4_operands[index]]

    for index in range(10):
        result[f"r5-{index}"] = [(0, HEADER.size), (ckir_at, IR20.HEADER.size),
                                 op_chunks[index], operand_chunks[index],
                                 chunks(*coff["terminators"], 10)[index]]
    elf_at, elf_len = ext["elf"]
    result["r5-elf"] = [(0, HEADER.size), (ckir_at, IR20.HEADER.size),
                        (elf_at, min(512, elf_len))]
    return {name: merge(spans) for name, spans in result.items()}


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
    states = []; pieces = list(segments(frame, spans))
    for index, piece in enumerate(pieces):
        nxt = f"s{index + 1}" if index + 1 < len(pieces) else "accepted"
        if piece[0] == "run":
            _, start, end, value = piece
            states.append(f"state s{index} {{ i={start} to run{index} }}\n"
                          f"state run{index} {{ to {nxt} when(i>={end}) "
                          f"to bad when(fb(i)!={value}) i=i+1 to run{index} }}")
        else:
            checks = " ".join(f"to bad when(fb({at})!={value})"
                              for at, value in piece[1])
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
    for owner, spans in projections(ctx).items():
        (output / f"{owner}.beta").write_text(checker(frame, spans), encoding="ascii")
        reject_at = spans[-1][0]
        reject = bytearray(frame); reject[reject_at] ^= 1
        (output / f"{owner}-reject.rfn").write_bytes(reject)
        manifest.append(f"{owner}\t{reject_at}\n")
    (output / "manifest.tsv").write_text("".join(manifest), encoding="ascii")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(); parser.add_argument("output", type=Path)
    materialize(parser.parse_args().output)
