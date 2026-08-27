#!/usr/bin/env python3
"""Materialize small seed-lineage joins for representative OMGRFN17 carriers.

These programs are deliberately representative component-projection witnesses,
not an alternate finite allowlist for the general Python acceptance relation.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from omgrfn17_frame import HEADER
from omgrfn17_profiles import profiles

ASSIGNMENT = {
    "r1": "recurrent",
    "r2": "recurrent",
    "r3": "recurrent",
    "r4-lowering-source": "recurrent",
    "r4-lowering-ckir": "recurrent",
    "r4-source-result": "empty",
    "r5-structure": "one-byte",
    "r5-result": "one-byte",
    "r5-elf-ckir": "recurrent",
    "r5-elf-image": "recurrent",
}


def extents(frame: bytes) -> dict[str, tuple[int, int]]:
    fields = HEADER.unpack_from(frame)
    at = HEADER.size; result = {"header": (0, HEADER.size)}
    for name, length in zip(("omgcomp", "witness", "ckir", "elf"), fields[3:7]):
        result[name] = (at, length); at += length
    return result


def owned(owner: str, frame: bytes) -> tuple[tuple[int, int], ...]:
    spans = extents(frame)
    result_claim = (32, 8)
    return {
        "r1": ((0, HEADER.size + spans["omgcomp"][1]),),
        "r2": (spans["omgcomp"], spans["witness"]),
        "r3": (spans["ckir"],),
        "r4-lowering-source": (spans["omgcomp"],),
        "r4-lowering-ckir": (spans["ckir"],),
        "r4-source-result": (spans["omgcomp"], result_claim),
        "r5-structure": (spans["ckir"],),
        "r5-result": (spans["ckir"], result_claim),
        "r5-elf-ckir": (spans["ckir"],),
        "r5-elf-image": (spans["elf"],),
    }[owner]


def segments(frame: bytes, spans: tuple[tuple[int, int], ...]):
    for start, length in spans:
        end = start + length; cursor = start; literals: list[tuple[int, int]] = []
        while cursor < end:
            run = 1
            while cursor + run < end and frame[cursor + run] == frame[cursor]:
                run += 1
            if run >= 8:
                if literals:
                    yield ("literal", tuple(literals)); literals = []
                yield ("run", cursor, cursor + run, frame[cursor]); cursor += run
            else:
                literals.append((cursor, frame[cursor])); cursor += 1
                if len(literals) == 20:
                    yield ("literal", tuple(literals)); literals = []
        if literals:
            yield ("literal", tuple(literals))


def checker(owner: str, frame: bytes) -> str:
    pieces = list(segments(frame, owned(owner, frame)))
    states = []
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
        to bad when(fb(6)!='H') to bad when(fb(7)!=0)
        to bad when(fb(8)!=17) to bad when(fb(9)!=0) to bad when(fb(10)!=0) to bad when(fb(11)!=0)
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


def materialize(output: Path, resolver: Path | None = None) -> None:
    output.mkdir(parents=True, exist_ok=True)
    carriers = profiles(resolver)
    manifest = []
    for name, frame in carriers.items():
        (output / f"{name}.rfn").write_bytes(frame)
    for owner, profile in ASSIGNMENT.items():
        frame = carriers[profile]
        (output / f"{owner}.beta").write_text(checker(owner, frame), encoding="ascii")
        reject = bytearray(frame)
        reject_at = owned(owner, frame)[-1][0]
        reject[reject_at] ^= 1
        (output / f"{owner}-reject.rfn").write_bytes(reject)
        manifest.append(f"{owner}\t{profile}\t{reject_at}\n")
    (output / "manifest.tsv").write_text("".join(manifest), encoding="ascii")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(); parser.add_argument("output", type=Path)
    parser.add_argument("--resolver", type=Path)
    args = parser.parse_args()
    materialize(args.output, args.resolver)
