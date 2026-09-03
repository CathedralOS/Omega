#!/usr/bin/env python3
"""Recover symbolic labels from the retained addressed Forth-Gamma evaluator."""

import re
import sys
from pathlib import Path

LABEL = re.compile(r"^0x[0-9a-f]+: ; ([a-z][a-z0-9_]*):?(.*)$")
TARGET = re.compile(r"(0x[0-9a-f]+)(\s*;\s*->\s*([a-z][a-z0-9_]*):)")


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: import_legacy.py INPUT.beta OUTPUT.sbeta")

    output = []
    for number, line in enumerate(Path(sys.argv[1]).read_text().splitlines(), 1):
        label = LABEL.fullmatch(line)
        if label:
            suffix = label.group(2).strip()
            output.append(f"{label.group(1)}:" + (f" ; {suffix}" if suffix else ""))
            continue

        target = TARGET.search(line)
        if target:
            name = target.group(3)
            line = line[: target.start()] + f"@{name}" + line[target.end() :]
        elif re.match(r"\s*(?:jmp|jz|jnz|jlt|jeq|call)\b", line) and "0x" in line:
            raise SystemExit(f"unlabeled control target on line {number}")
        output.append(line)

    Path(sys.argv[2]).write_text("\n".join(output) + "\n")


if __name__ == "__main__":
    main()
