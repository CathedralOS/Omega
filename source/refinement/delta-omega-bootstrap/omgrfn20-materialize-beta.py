#!/usr/bin/env python3
"""Materialize focused persisted-Beta projections for OMGRFN20 owners."""

from __future__ import annotations

import argparse
import importlib.util
import sys
from pathlib import Path

from omgrfn19_witness import ROWS as WITNESS_ROWS, decode as decode_witness
from omgrfn20_bundle import pack
from omgrfn20_ckir import reference as ir17
from omgrfn20_frame import HEADER
from omgrfn20_source import decode_sources

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
GATES = ROOT / "source/on-ramp/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def canonical():
    v9 = load("omgrfn20_beta_v9", GATES / "omgrsw9_provider_plan_reference.py")
    fixture = load("omgrfn20_beta_ckir", GATES / "delta-checked-ir-v17-fixture.py")
    envelope = v9.encode_envelope(v9.fixture_contents())
    witness = v9.encode_witness(envelope)
    ckir = fixture.encode(fixture.tables())
    frame = pack(envelope, witness, ckir)
    holder = type("Frame", (), {"omgcomp": envelope})()
    _, sources = decode_sources(holder)
    parsed_witness = decode_witness(witness)
    parsed_ckir = ir17.decode(ckir)
    witness_at = HEADER.size + len(envelope)
    ckir_at = witness_at + len(witness)
    source_at = []
    for source in sources:
        at = frame.find(source, HEADER.size, witness_at)
        if at < 0 or frame.find(source, at + 1, witness_at) >= 0:
            raise RuntimeError("source content position")
        source_at.append(at)
    ckir_offsets = {}
    cursor = ir17.HEADER.size
    for name in ir17.TABLE_ORDER:
        length = len(parsed_ckir.tables[name]) * ir17.ROWS[name].size
        ckir_offsets[name] = (ckir_at + cursor, length)
        cursor += length
    return (frame, sources, parsed_witness, parsed_ckir, witness_at, ckir_at,
            tuple(source_at), ckir_offsets)


def wrow(parsed, witness_at: int, name: str, row: int) -> tuple[int, int]:
    start, _ = parsed.offsets[name]
    size = WITNESS_ROWS[name].size
    return witness_at + start + row * size, size


def crow(offsets, name: str, row: int) -> tuple[int, int]:
    start, _ = offsets[name]
    size = ir17.ROWS[name].size
    return start + row * size, size


def srow(source_at, source: int, start: int, length: int) -> tuple[int, int]:
    return source_at[source] + start, length


def owned(context, owner: str) -> tuple[tuple[int, int], ...]:
    frame, sources, witness, ckir, witness_at, ckir_at, source_at, offsets = context
    spans: list[tuple[int, int]] = [(0, HEADER.size)]
    if owner == "r1":
        spans.extend(((HEADER.size, 64), (witness_at, 144), (ckir_at, ir17.HEADER.size)))
    elif owner == "r2":
        for name, rows in (("providers", (0,)), ("helpers", (0,)),
                           ("adapters", (0, 1)), ("candidates", (1, 4)),
                           ("plans", (0,)), ("plan_rows", (1, 4)),
                           ("requirement_calls", (0, 1)),
                           ("ordinary_calls", (0, 1))):
            spans.extend(wrow(witness, witness_at, name, row) for row in rows)
        helper = witness.tables["helpers"][0]
        spans.append(srow(source_at, helper[1], helper[10], helper[11]))
        for row in witness.tables["adapters"]:
            spans.append(srow(source_at, row[1], row[10], row[11]))
    elif owner == "r3":
        # R3's executable owner decodes and checks the complete CKIR17 carrier.
        # Keep the persisted-Beta projection representative and focused: bind
        # the exact header plus every row that carries R3's service, machine,
        # reach, ranking, block-shape, and opcode responsibilities.  Copying
        # the operand/value scaffolding here adds no independent proposition
        # and pushes the seed tape over the architectural ceiling.
        for name in ("machines", "machine_params", "blocks", "operations",
                     "services", "machine_reaches", "rankings",
                     "boundary_targets"):
            spans.append(offsets[name])
    elif owner == "r4":
        for name, rows in (("helpers", (0,)), ("adapters", (0, 1)),
                           ("plan_rows", (1, 4)), ("requirement_calls", (0, 1))):
            spans.extend(wrow(witness, witness_at, name, row) for row in rows)
        for name, rows in (("services", (0,)), ("machine_reaches", (0, 1, 2)),
                           ("rankings", (0,)), ("boundary_targets", (0,)),
                           ("machines", (0, 1, 2)),
                           ("operations", (3, 4, 9, 10, 12, 14))):
            spans.extend(crow(offsets, name, row) for row in rows)
        for row in witness.tables["requirement_calls"][:2]:
            spans.append(srow(source_at, row[1], row[5], row[6]))
    elif owner == "r5":
        # R5's Beta tooth binds the executable trace spine.  Type/machine and
        # block-parameter declarations are already projected by R3; retaining
        # them here would duplicate structural custody and exceed the seed
        # tape ceiling without strengthening the independent observation.
        for name in ("machine_params", "blocks", "operations", "operands",
                     "terminators", "services",
                     "machine_reaches", "rankings", "boundary_targets"):
            spans.append(offsets[name])
    else:
        raise RuntimeError(owner)
    return tuple(spans)


def segments(frame: bytes, spans):
    for start, length in spans:
        end = start + length
        cursor = start
        literals = []
        while cursor < end:
            run = 1
            while cursor + run < end and frame[cursor + run] == frame[cursor]:
                run += 1
            if run >= 8:
                if literals:
                    yield "literal", tuple(literals)
                    literals = []
                yield "run", cursor, cursor + run, frame[cursor]
                cursor += run
            else:
                literals.append((cursor, frame[cursor]))
                cursor += 1
                if len(literals) == 20:
                    yield "literal", tuple(literals)
                    literals = []
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
                          f"    state run{index} {{ to {nxt} when(i>={end}) "
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
    context = canonical()
    frame = context[0]
    (output / "canonical.rfn").write_bytes(frame)
    manifest = []
    for owner in ("r1", "r2", "r3", "r4", "r5"):
        spans = owned(context, owner)
        (output / f"{owner}.beta").write_text(checker(frame, spans), encoding="ascii")
        reject_at = spans[-1][0]
        reject = bytearray(frame)
        reject[reject_at] ^= 1
        (output / f"{owner}-reject.rfn").write_bytes(reject)
        manifest.append(f"{owner}\t{reject_at}\n")
    (output / "manifest.tsv").write_text("".join(manifest), encoding="ascii")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    materialize(parser.parse_args().output)
