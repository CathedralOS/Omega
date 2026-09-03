#!/usr/bin/env python3
"""Resolve test-owned symbolic labels into canonical addressed Beta."""

import re
import sys
from pathlib import Path

WIDTHS = {
    "halt": 2,
    "imm": 10,
    "mov": 3,
    "add": 3,
    "sub": 3,
    "mul": 3,
    "div": 3,
    "mod": 3,
    "loadb": 3,
    "storeb": 3,
    "load": 3,
    "store": 3,
    "jmp": 9,
    "jz": 10,
    "jnz": 10,
    "jlt": 11,
    "jeq": 11,
    "read": 2,
    "write": 2,
    "call": 9,
    "ret": 1,
    "dw": 8,
}
LABEL = re.compile(r"^([a-z][a-z0-9_]*):(?:\s*;.*)?$")
REFERENCE = re.compile(r"@([a-z][a-z0-9_]*)")


def instruction_width(line: str) -> int:
    code = line.split(";", 1)[0].strip()
    if not code or LABEL.fullmatch(line.strip()):
        return 0
    mnemonic = code.split()[0]
    try:
        return WIDTHS[mnemonic]
    except KeyError as error:
        raise SystemExit(f"unknown symbolic Beta line: {line}") from error


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: resolve.py INPUT.sbeta OUTPUT.beta")
    lines = Path(sys.argv[1]).read_text().splitlines()
    labels = {}
    pc = 0
    for number, line in enumerate(lines, 1):
        match = LABEL.fullmatch(line.strip())
        if match:
            name = match.group(1)
            if name in labels:
                raise SystemExit(f"duplicate label {name} on line {number}")
            labels[name] = pc
        else:
            pc += instruction_width(line)

    output = []
    for number, line in enumerate(lines, 1):
        match = LABEL.fullmatch(line.strip())
        if match:
            output.append(f"0x{labels[match.group(1)]:x}: ; {match.group(1)}:")
            continue

        references = []

        def replace(reference: re.Match[str]) -> str:
            name = reference.group(1)
            if name not in labels:
                raise SystemExit(f"unknown label {name} on line {number}")
            references.append(name)
            return f"0x{labels[name]:x}"

        resolved = REFERENCE.sub(replace, line)
        if references:
            destinations = ", ".join(f"{name}:" for name in references)
            resolved += f" ; -> {destinations}"
        output.append(resolved)
    Path(sys.argv[2]).write_text("\n".join(output) + "\n")


if __name__ == "__main__":
    main()
