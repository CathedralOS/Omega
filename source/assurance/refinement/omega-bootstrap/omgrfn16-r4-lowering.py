#!/usr/bin/env python3
"""OMGRFN16 R4 source-token to CKIR14 postorder lowering owner."""

from __future__ import annotations

import collections
import sys

from omgrfn16_ckir import ARITHMETIC, V5, check_arithmetic_closure, decode
from omgrfn16_frame import RefinementError, RefinementResourceError, require, split
from omgrfn16_source import Expr, selected_run, source_contents


def walk(expression: Expr, literals: list[int], leaves: list[bytes]) -> None:
    if expression.left is not None:
        walk(expression.left, literals, leaves)
    if expression.right is not None:
        walk(expression.right, literals, leaves)
    if expression.kind == "literal":
        literals.append(int(expression.value))
    elif expression.kind == "leaf":
        leaves.append(bytes(expression.value))


def main() -> None:
    frame = split(sys.stdin.buffer.read())
    sources = source_contents(frame.omgcomp)
    candidates = [source for source in sources if b"::run" in source]
    require(len(candidates) == 1, "unique selected source unit")
    program = selected_run(candidates[0])
    module = decode(frame.ckir)
    check_arithmetic_closure(module)

    operations = module.tables["operations"]
    actual = tuple(row[3] for row in operations if row[3] in ARITHMETIC)
    require(actual == program.postorder(), "exact authored postorder operation join")
    require(sum(row[3] == 21 for row in operations) == program.widen_count(),
            "exact authored widening join")

    literals: list[int] = []
    leaves: list[bytes] = []
    for expression in program.expressions:
        walk(expression, literals, leaves)
    constants = collections.Counter(row[10] for row in operations if row[3] == 1)
    require(not (collections.Counter(literals) - constants), "literal-to-value custody")
    direct_leaves = [leaf for leaf in leaves if leaf.startswith(b"self.")]
    require(sum(row[3] == 5 for row in operations) >= len(direct_leaves),
            "direct-load leaf custody")


if __name__ == "__main__":
    try:
        main()
    except (RefinementResourceError, V5.Ckir5ResourceError) as error:
        print(f"OMGRFN16 R4-lowering: {error}", file=sys.stderr)
        raise SystemExit(252)
    except (RefinementError, OSError) as error:
        print(f"OMGRFN16 R4-lowering: {error}", file=sys.stderr)
        raise SystemExit(251)
