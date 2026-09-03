#!/usr/bin/env python3
"""Report stable audit-surface metrics for the two Gamma/Delta routes."""

import re
import sys
from pathlib import Path

LABEL = re.compile(r"[a-z][a-z0-9_]*:")


def beta_metrics(path: str) -> tuple[int, int, int, int, int, int]:
    lines = Path(path).read_text().splitlines()
    instructions = []
    labels = 0
    for line in lines:
        code = line.split(";", 1)[0].strip()
        if LABEL.fullmatch(code):
            labels += 1
        elif code:
            instructions.append(code.split()[0].rstrip(","))
    branches = sum(op in {"jmp", "jz", "jnz", "jlt", "jeq"} for op in instructions)
    calls = instructions.count("call")
    control = branches + calls + instructions.count("ret")
    return len(lines), len(instructions), labels, control, branches, calls


def forth_tokens(path: str) -> list[str]:
    source = Path(path).read_text()
    tokens = []
    index = 0
    while index < len(source):
        byte = source[index]
        if byte.isspace():
            index += 1
        elif byte == "#":
            newline = source.find("\n", index)
            index = len(source) if newline < 0 else newline + 1
        elif byte == '"':
            end = source.find('"', index + 1)
            if end < 0:
                raise SystemExit("unterminated measured text token")
            tokens.append("<text>")
            index = end + 1
        else:
            end = index + 1
            while end < len(source) and not source[end].isspace():
                end += 1
            tokens.append(source[index:end])
            index = end
    return tokens


def main() -> None:
    if len(sys.argv) != 5:
        raise SystemExit(
            "usage: measure.py FORTH_EVALUATOR FORTH_COMPILER "
            "FUNCTIONAL_EVALUATOR FUNCTIONAL_COMPILER"
        )
    forth_evaluator, forth_compiler, functional_evaluator, functional_compiler = sys.argv[1:]
    forth = forth_tokens(forth_compiler)
    functional = Path(functional_compiler).read_text()

    print("forth_beta=" + ",".join(map(str, beta_metrics(forth_evaluator))))
    print(f"forth_compiler_lines={len(Path(forth_compiler).read_text().splitlines())}")
    print(f"forth_compiler_definitions={forth.count(':')}")
    print(f"forth_compiler_values={forth.count('value')}")
    print(f"forth_compiler_tokens={len(forth)}")
    print(f"forth_compiler_branches={forth.count('branch')}")
    print(f"forth_compiler_jumps={forth.count('jump')}")
    print(
        "forth_compiler_stack_ops="
        + str(sum(forth.count(op) for op in ("dup", "swap", "over", "drop")))
    )
    print(
        "forth_compiler_cell_ops="
        + str(forth.count("cell-get") + forth.count("cell-set"))
    )
    print("functional_beta=" + ",".join(map(str, beta_metrics(functional_evaluator))))
    print(f"functional_compiler_lines={len(functional.splitlines())}")
    print(f"functional_compiler_definitions={len(re.findall(r'^\(def ', functional, re.MULTILINE))}")
    print(f"functional_compiler_lets={len(re.findall(r'\(let\b', functional))}")


if __name__ == "__main__":
    main()
