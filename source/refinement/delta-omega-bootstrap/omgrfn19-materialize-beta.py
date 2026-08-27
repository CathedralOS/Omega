#!/usr/bin/env python3
"""Materialize representative persisted-Beta projections for OMGRFN19.

The generated programs bind selected source bytes and the wire rows owned by
each general Python responsibility.  They are focused lower-rooted projections,
not a finite replacement for the general structural relation.
"""

from __future__ import annotations

import argparse
import importlib.util
import sys
from pathlib import Path

from omgrfn19_bundle import pack
from omgrfn19_frame import HEADER
from omgrfn19_source import decode_sources
from omgrfn19_witness import decode


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
REFERENCE = ROOT / "source/on-ramp/omega-bootstrap/gates/omgrsw9_provider_plan_reference.py"


def load_reference():
    spec = importlib.util.spec_from_file_location("omgrsw9_v9_beta_reference", REFERENCE)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load OMGRSW9 reference")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def canonical():
    reference = load_reference()
    envelope = reference.encode_envelope(reference.fixture_contents())
    witness = reference.encode_witness(envelope)
    frame = pack(envelope, witness)
    # A tiny duck-typed frame is sufficient for the shared source projection.
    holder = type("Frame", (), {"omgcomp": envelope})()
    _, sources = decode_sources(holder)
    parsed = decode(witness)
    witness_at = HEADER.size + len(envelope)
    source_at = []
    for source in sources:
        at = frame.find(source, HEADER.size, witness_at)
        if at < 0 or frame.find(source, at + 1, witness_at) >= 0:
            raise RuntimeError("source content does not have one frame position")
        source_at.append(at)
    return frame, sources, parsed, witness_at, tuple(source_at)


def table_span(parsed, witness_at: int, name: str) -> tuple[int, int]:
    start, length = parsed.offsets[name]
    return witness_at + start, length


def source_span(source_at, source: int, start: int, length: int) -> tuple[int, int]:
    return source_at[source] + start, length


def owned(frame: bytes, sources, parsed, witness_at: int, source_at,
          owner: str) -> tuple[tuple[int, int], ...]:
    spans: list[tuple[int, int]] = [(0, HEADER.size)]
    if owner == "r1":
        spans.append((HEADER.size, 64))  # complete OMGCOMP3 fixed header
        spans.append((witness_at, 144))  # complete OMGRSW9 fixed header
    elif owner == "r2":
        spans.append((source_at[parsed.build_source], len(sources[parsed.build_source])))
        for name in ("units", "build_machines", "selections"):
            spans.append(table_span(parsed, witness_at, name))
    elif owner == "r3":
        for name in ("types", "traits", "requirements", "requirement_params", "reaches"):
            spans.append(table_span(parsed, witness_at, name))
        trait = parsed.tables["traits"][0]
        spans.append(source_span(source_at, trait[1], trait[3], trait[4]))
        for table, fields in (("requirements", (3, 4, 5)),
                              ("requirement_params", (None, 4, 5)),
                              ("reaches", (3, 4, 5))):
            for row in parsed.tables[table]:
                source = 0 if fields[0] is None else row[fields[0]]
                spans.append(source_span(source_at, source, row[fields[1]], row[fields[2]]))
    elif owner == "r4":
        for name in ("providers", "helpers", "adapters", "candidates",
                     "candidate_params", "ordinary_calls"):
            spans.append(table_span(parsed, witness_at, name))
        helper = parsed.tables["helpers"][0]
        spans.append(source_span(source_at, helper[1], helper[10], helper[11]))
        for row in parsed.tables["adapters"]:
            spans.append(source_span(source_at, row[1], row[10], row[11]))
        for row in parsed.tables["ordinary_calls"]:
            spans.append(source_span(source_at, row[1], row[5], row[6]))
    elif owner == "r5":
        for name in ("plans", "plan_rows", "requirement_calls"):
            spans.append(table_span(parsed, witness_at, name))
        for row in parsed.tables["requirement_calls"]:
            spans.append(source_span(source_at, row[1], row[5], row[6]))
    else:
        raise RuntimeError(f"unknown owner {owner}")
    return tuple(spans)


def segments(frame: bytes, spans: tuple[tuple[int, int], ...]):
    for start, length in spans:
        end = start + length
        cursor = start
        literals: list[tuple[int, int]] = []
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


def checker(owner: str, frame: bytes, spans: tuple[tuple[int, int], ...]) -> str:
    pieces = list(segments(frame, spans))
    states: list[str] = []
    for index, piece in enumerate(pieces):
        next_state = f"s{index + 1}" if index + 1 < len(pieces) else "accepted"
        if piece[0] == "run":
            _, start, end, value = piece
            states.append(
                f"state s{index} {{ i={start} to run{index} }}\n"
                f"    state run{index} {{ to {next_state} when(i>={end}) "
                f"to bad when(fb(i)!={value}) i=i+1 to run{index} }}"
            )
        else:
            checks = " ".join(f"to bad when(fb({at})!={value})" for at, value in piece[1])
            states.append(f"state s{index} {{ {checks} to {next_state} }}")
    expected = len(frame)
    return f'''proc fb(index) {{ return byte[1048576+index] }}
proc read_frame() {{
    let n=0 let c=read_byte()
    state read {{ to one when(c>=0) to finish }}
    state one {{ to bad when(n>={expected}) byte[1048576+n]=c n=n+1 c=read_byte() to read }}
    state finish {{
        to bad when(n!={expected})
        to bad when(fb(0)!='O') to bad when(fb(1)!='M') to bad when(fb(2)!='G')
        to bad when(fb(3)!='R') to bad when(fb(4)!='F') to bad when(fb(5)!='N')
        to bad when(fb(6)!='J') to bad when(fb(7)!=0)
        to bad when(fb(8)!=19) to bad when(fb(9)!=0) to bad when(fb(10)!=0) to bad when(fb(11)!=0)
        return 0
    }}
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
    frame, sources, parsed, witness_at, source_at = canonical()
    (output / "canonical.rfn").write_bytes(frame)
    manifest: list[str] = []
    for owner in ("r1", "r2", "r3", "r4", "r5"):
        spans = owned(frame, sources, parsed, witness_at, source_at, owner)
        (output / f"{owner}.beta").write_text(checker(owner, frame, spans), encoding="ascii")
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
