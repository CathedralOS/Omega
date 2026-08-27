#!/usr/bin/env python3
"""Materialize split representative persisted-Beta projections for OMGRFN22."""

from __future__ import annotations

import argparse
from pathlib import Path

from omgrfn22_ckir import IR19, arguments, producer_decode
from omgrfn22_frame import HEADER
from omgrfn22_profiles import canonical
from omgrfn22_source import ROWS as WROWS
from omgrfn22_source import decode_witness, source_closure


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
    offsets = {}; cursor = ckir_at + IR19.HEADER.size
    for name in IR19.TABLE_ORDER:
        extent = len(module.tables[name]) * IR19.ROWS[name].size
        offsets[name] = (cursor, extent); cursor += extent
    return frame, extents, witness, source_at, module, offsets


def wrow(witness, base: int, name: str, row: int) -> tuple[int, int]:
    return (base + witness.offsets[name][0] + row * WROWS[name].size,
            WROWS[name].size)


def crow(offsets, name: str, row: int) -> tuple[int, int]:
    return (offsets[name][0] + row * IR19.ROWS[name].size,
            IR19.ROWS[name].size)


def op_spans(module, offsets, ids) -> list[tuple[int, int]]:
    result = []
    for op_id in sorted(ids):
        operation = module.tables["operations"][op_id]
        result.append(crow(offsets, "operations", op_id))
        if operation[9]:
            result.append((offsets["operands"][0] + operation[8] * 4,
                           operation[9] * 4))
    return result


def merge(spans: list[tuple[int, int]]) -> tuple[tuple[int, int], ...]:
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


def owned(ctx, owner: str) -> tuple[tuple[int, int], ...]:
    frame, extents, witness, source_at, module, offsets = ctx
    wit_at = extents["witness"][0]; ckir_at = extents["ckir"][0]
    operations = module.tables["operations"]
    by_opcode = {opcode: {row[0] for row in operations if row[3] == opcode}
                 for opcode in range(1, 11)}
    spans = [(0, HEADER.size)]
    if owner == "r1":
        spans += [(extents[name][0], min(extents[name][1], 160))
                  for name in ("omgcomp", "witness", "ckir", "elf")]
    elif owner == "r2-decls":
        spans += [(wit_at, 160), (wit_at + witness.offsets["types"][0],
                                  witness.offsets["types"][1]),
                  (wit_at + witness.offsets["records"][0],
                   witness.offsets["records"][1]),
                  (wit_at + witness.offsets["fields"][0],
                   witness.offsets["fields"][1]),
                  (source_at, 601)]
    elif owner == "r2-tables":
        for name in ("machines", "params", "blocks", "calls", "stores", "arguments"):
            spans.append((wit_at + witness.offsets[name][0], witness.offsets[name][1]))
    elif owner in ("r2-push-a", "r2-push-b"):
        row = witness.tables["machines"][0]; half = row[12] // 2
        relative = 0 if owner.endswith("-a") else half
        length = half if owner.endswith("-a") else row[12] - half
        spans.append((source_at + row[11] + relative, length))
        store_rows = range(0, 5) if owner.endswith("-a") else range(5, 9)
        spans += [wrow(witness, wit_at, "stores", index) for index in store_rows]
    elif owner == "r2-read-root":
        for row in witness.tables["machines"][1:]:
            spans.append((source_at + row[11], row[12]))
        spans.append((wit_at + witness.offsets["calls"][0], witness.offsets["calls"][1]))
        spans.append((wit_at + witness.offsets["arguments"][0], witness.offsets["arguments"][1]))
    elif owner == "r3-layout":
        spans.append((ckir_at, IR19.HEADER.size))
        for name in ("types", "records", "fields", "machines", "machine_params", "blocks"):
            spans.append(offsets[name])
    elif owner in ("r3-places-a", "r3-places-b", "r3-places-c"):
        selected = sorted(by_opcode[2] | by_opcode[3] | by_opcode[4]
                          | by_opcode[5] | by_opcode[6])
        cuts = (0, (len(selected) + 2) // 3, (2 * len(selected) + 2) // 3,
                len(selected))
        part = {"r3-places-a": 0, "r3-places-b": 1, "r3-places-c": 2}[owner]
        spans += op_spans(module, offsets, selected[cuts[part]:cuts[part + 1]])
    elif owner == "r3-control":
        spans += op_spans(module, offsets,
                          by_opcode[1] | by_opcode[8] | by_opcode[9] | by_opcode[10])
        spans.append(offsets["terminators"])
    elif owner in ("r4-store-a", "r4-store-b", "r4-store-c"):
        row = witness.tables["machines"][0]
        part = {"r4-store-a": 0, "r4-store-b": 1, "r4-store-c": 2}[owner]
        cuts = (0, (row[12] + 2) // 3, (2 * row[12] + 2) // 3, row[12])
        relative, length = cuts[part], cuts[part + 1] - cuts[part]
        spans.append((source_at + row[11] + relative, length))
        store_cuts = (0, 3, 6, 9)
        stores = range(store_cuts[part], store_cuts[part + 1])
        spans += [wrow(witness, wit_at, "stores", index) for index in stores]
        selected = sorted(row[0] for row in operations if row[1] == 0 and row[2] == 1
                          and row[3] in (3, 4, 5, 6))
        op_cuts = (0, (len(selected) + 2) // 3,
                   (2 * len(selected) + 2) // 3, len(selected))
        spans += op_spans(module, offsets, selected[op_cuts[part]:op_cuts[part + 1]])
    elif owner == "r4-flow":
        for row in (4, 5, 6):
            spans.append(wrow(witness, wit_at, "types", row))
        spans.append((wit_at + witness.offsets["blocks"][0], witness.offsets["blocks"][1]))
        spans += op_spans(module, offsets, by_opcode[8] | by_opcode[9])
        spans.append(offsets["terminators"])
    elif owner == "r4-calls":
        row = witness.tables["machines"][2]
        spans.append((source_at + row[11], row[12]))
        spans.append((wit_at + witness.offsets["calls"][0], witness.offsets["calls"][1]))
        spans.append((wit_at + witness.offsets["arguments"][0], witness.offsets["arguments"][1]))
        spans += op_spans(module, offsets,
                          {row[0] for row in operations if row[1] == 2})
    elif owner in ("r5-writer-a", "r5-writer-b", "r5-writer-c"):
        selected = sorted(row[0] for row in operations if row[1] == 0)
        part = {"r5-writer-a": 0, "r5-writer-b": 1, "r5-writer-c": 2}[owner]
        cuts = (0, (len(selected) + 2) // 3,
                (2 * len(selected) + 2) // 3, len(selected))
        spans.append((ckir_at, IR19.HEADER.size)); spans.append(offsets["machines"])
        spans += op_spans(module, offsets, selected[cuts[part]:cuts[part + 1]])
        spans += [crow(offsets, "terminators", row) for row in range(3)]
    elif owner == "r5-read-run":
        spans.append((ckir_at, IR19.HEADER.size)); spans.append(offsets["machines"])
        spans += op_spans(module, offsets,
                          {row[0] for row in operations if row[1] in (1, 2)})
        spans += [crow(offsets, "terminators", row) for row in range(3, 7)]
    elif owner == "r5-elf":
        elf_at, elf_len = extents["elf"]
        spans += [(ckir_at, IR19.HEADER.size), (elf_at, 256)]
        elf = frame[elf_at:elf_at + elf_len]
        for needle in (b"\x48\x69\xc0\x28\x00\x00\x00", b"\x0f\x80",
                       b"\x49\x01\xc2", b"\x0f\x82",
                       b"\x48\x05\x20\x00\x00\x00"):
            at = elf.find(needle)
            if at < 0:
                raise RuntimeError(f"missing ELF projection {needle!r}")
            spans.append((elf_at + max(0, at - 12),
                          min(48, elf_len - max(0, at - 12))))
    else:
        raise RuntimeError(owner)
    return merge(spans)


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
    for owner in ("r1", "r2-decls", "r2-tables", "r2-push-a", "r2-push-b",
                  "r2-read-root", "r3-layout", "r3-places-a", "r3-places-b",
                  "r3-places-c", "r3-control", "r4-store-a", "r4-store-b",
                  "r4-store-c", "r4-flow",
                  "r4-calls", "r5-writer-a", "r5-writer-b", "r5-writer-c",
                  "r5-read-run",
                  "r5-elf"):
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
