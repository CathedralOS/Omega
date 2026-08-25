#!/usr/bin/env python3
"""Pure source materializer for the shared OMGRFN8/9/10 R1/R2 Beta checkers."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


HERE = Path(__file__).resolve().parent


def replace_exact(source: str, old: str, new: str, count: int = 1) -> str:
    actual = source.count(old)
    if actual != count:
        raise ValueError(f"materialization anchor count {actual}, expected {count}: {old}")
    return source.replace(old, new)


def dynamic_source_core(source: str) -> str:
    """Bound the inherited focused OMGCOMP decoder to one or two source rows."""
    replacements = (
        (
            "to bad when (word[500320]!=2)",
            "to bad when (word[500320]<1) to bad when (word[500320]>2)",
            1,
        ),
        (
            "to bad when (word[500336]!=0) to bad when (word[500344]!=1)",
            "to bad when (word[500336]!=0) to bad when (word[500344]>=word[500320])",
            2,
        ),
        (
            "word[500368]=64 word[500376]=112 word[500384]=152 word[500392]=152",
            "word[500368]=64 word[500376]=112 word[500384]=112+word[500320]*20 word[500392]=word[500384]",
            1,
        ),
        (
            "to bad when (omgrfn4_l2_comp_u32(104)!=2)",
            "to bad when (omgrfn4_l2_comp_u32(104)!=word[500320])",
            1,
        ),
        (
            "state sources { to source_row when (i<2)",
            "state sources { to source_row when (i<word[500320])",
            1,
        ),
        (
            "to bad when (bundle>=2)",
            "to bad when (bundle>=word[500320])",
            1,
        ),
        (
            "to bad when (omgrfn4_l2_comp_u32(at+12)!=2)",
            "to bad when (omgrfn4_l2_comp_u32(at+12)!=word[500320])",
            1,
        ),
        (
            "state bundle_rows { to bundle_row when (i<2)",
            "state bundle_rows { to bundle_row when (i<word[500320])",
            1,
        ),
        (
            "state source_extents { to source_extent when (i<2)",
            "state source_extents { to source_extent when (i<word[500320])",
            1,
        ),
    )
    for old, new, count in replacements:
        source = replace_exact(source, old, new, count)
    return source


def procedures(source: str) -> tuple[list[str], dict[str, str]]:
    order: list[str] = []
    bodies: dict[str, str] = {}
    pattern = re.compile(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\([^)]*\)\s*\{")
    for match in pattern.finditer(source):
        depth = 1
        cursor = match.end()
        while depth:
            if cursor >= len(source):
                raise ValueError(f"unterminated procedure {match.group(1)}")
            depth += (source[cursor] == "{") - (source[cursor] == "}")
            cursor += 1
        name = match.group(1)
        if name in bodies:
            raise ValueError(f"duplicate procedure {name}")
        order.append(name)
        bodies[name] = source[match.start():cursor].rstrip() + "\n"
    return order, bodies


def prune(source: str, entry: str = "main") -> str:
    order, bodies = procedures(source)
    reachable: set[str] = set()
    pending = [entry]
    while pending:
        name = pending.pop()
        if name in reachable:
            continue
        if name not in bodies:
            raise ValueError(f"missing reachable procedure {name}")
        reachable.add(name)
        for called in re.findall(r"\b([A-Za-z_]\w*)\s*\(", bodies[name]):
            if called in bodies and called not in reachable:
                pending.append(called)
    return "\n".join(bodies[name] for name in order if name in reachable)


def shape(source: str) -> tuple[int, int]:
    order, bodies = procedures(source)
    maximum = 0
    for name in order:
        body = bodies[name]
        header = re.match(r"proc\s+\w+\(([^)]*)\)", body)
        assert header is not None
        params = sum(bool(item.strip()) for item in header.group(1).split(","))
        maximum = max(maximum, params + len(re.findall(r"\blet\s+[A-Za-z_]\w*", body)))
    return len(order), maximum


def write_checked(path: Path, source: str) -> tuple[int, int]:
    proc_count, max_locals = shape(source)
    if proc_count > 128 or max_locals > 32:
        raise ValueError(f"{path.name} exceeds Beta shape: {proc_count}/128 procedures, {max_locals}/32 locals")
    for forbidden in ("refinement_ckir_byte", "refinement_elf_byte"):
        if forbidden in source:
            raise ValueError(f"{path.name} gained artifact access through {forbidden}")
    path.write_text(source, encoding="ascii")
    return proc_count, max_locals


def materialize(output: Path, outer: int = 8) -> None:
    if outer not in (8, 9, 10, 11, 12):
        raise ValueError(f"unsupported OMGRFN outer version: {outer}")
    output.mkdir(parents=True, exist_ok=True)

    r1 = (
        (HERE / "omgrfn4-frame-omgcomp-custody.beta").read_text(encoding="ascii")
        + "\n"
        + (HERE / "omgrfn5-frame-omgcomp-custody.beta").read_text(encoding="ascii")
        + f"\nproc main() {{ return omgrfn{outer}_layer1_check() }}\n"
    )
    r2_core = dynamic_source_core(
        (HERE / "omgrfn4-source-witness-independent.beta").read_text(encoding="ascii")
    )
    r2 = prune(
        r2_core
        + "\n"
        + (HERE / "omgrfn7-source-witness-independent.beta").read_text(encoding="ascii")
        + f"\nproc main() {{ return omgrfn{outer}_r2_check() }}\n"
    )
    r1_shape = write_checked(output / "r1.beta", r1)
    r2_shape = write_checked(output / "r2.beta", r2)
    (output / "manifest.tsv").write_text(
        f"r1\t{r1_shape[0]}\t{r1_shape[1]}\n"
        f"r2\t{r2_shape[0]}\t{r2_shape[1]}\n",
        encoding="ascii",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("--outer", type=int, choices=(8, 9, 10, 11, 12), default=8)
    args = parser.parse_args()
    materialize(args.output, args.outer)


if __name__ == "__main__":
    main()
