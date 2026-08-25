#!/usr/bin/env python3
"""Pure source materializer for OMGRFN8/9/10 R3, R4, and R5 Beta checkers."""

from __future__ import annotations

import argparse
import re
from pathlib import Path


HERE = Path(__file__).resolve().parent


def read(name: str) -> str:
    return (HERE / name).read_text(encoding="ascii")


def replace_exact(source: str, old: str, new: str, count: int = 1) -> str:
    actual = source.count(old)
    if actual != count:
        raise ValueError(f"anchor count {actual}, expected {count}: {old}")
    return source.replace(old, new)


def one_source_core(source: str) -> str:
    replacements = (
        ("to bad when (word[500320]!=2)", "to bad when (word[500320]!=1)", 1),
        (
            "to bad when (word[500336]!=0) to bad when (word[500344]!=1)",
            "to bad when (word[500336]!=0) to bad when (word[500344]!=0)",
            2,
        ),
        (
            "word[500368]=64 word[500376]=112 word[500384]=152 word[500392]=152",
            "word[500368]=64 word[500376]=112 word[500384]=132 word[500392]=132",
            1,
        ),
        ("to bad when (omgrfn4_l2_comp_u32(104)!=2)", "to bad when (omgrfn4_l2_comp_u32(104)!=1)", 1),
        ("state sources { to source_row when (i<2)", "state sources { to source_row when (i<1)", 1),
        ("to bad when (bundle>=2)", "to bad when (bundle>=1)", 1),
        (
            "to bad when (omgrfn4_l2_comp_u32(at+12)!=2)",
            "to bad when (omgrfn4_l2_comp_u32(at+12)!=1)",
            1,
        ),
        ("state bundle_rows { to bundle_row when (i<2)", "state bundle_rows { to bundle_row when (i<1)", 1),
        ("state source_extents { to source_extent when (i<2)", "state source_extents { to source_extent when (i<1)", 1),
    )
    for old, new, count in replacements:
        source = replace_exact(source, old, new, count)
    return source


def lean_result_declarations(source: str) -> str:
    source = re.sub(r"\s+omgrfn4_r2_add_decl\([^\n]+?\)\s+", " ", source)
    source = replace_exact(
        source,
        "omgrfn5_r2_parse_state(source_id,mid,ordinal)",
        "r47_parse_state(source_id,mid,ordinal)",
    )
    match = re.search(r"(?m)^proc omgrfn5_r2_scan_block\([^)]*\) \{", source)
    if match is None:
        raise ValueError("R4 block scanner anchor")
    depth = 1
    end = match.end()
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    lean = """proc omgrfn5_r2_scan_block(source_id,machine_id,block_id,allow_state) {
    let depth=0 let t=0 let count=word[880000+source_id*8]
    state loop { t=omgrfn4_r2_cur(source_id) to bad when (t>=count) to state_end when (allow_state==1) to close_check }
    state state_end { to close_check when (depth!=0) to found_state when (omgrfn4_r2_is_word(source_id,t,2)==1) to close_check }
    state close_check { to punctuation when (depth!=0) to close when (omgrfn4_r2_tkind(source_id,t)==125) to punctuation }
    state punctuation { to open when (omgrfn4_r2_tkind(source_id,t)==123) to nested_close when (omgrfn4_r2_tkind(source_id,t)==125) omgrfn4_r2_next(source_id) to loop }
    state open { depth=depth+1 omgrfn4_r2_next(source_id) to loop }
    state nested_close { depth=depth-1 omgrfn4_r2_next(source_id) to loop }
    state found_state { word[888032+block_id*96]=omgrfn4_r2_tstart(source_id,t) word[879300]=1 return 0 }
    state close { word[888032+block_id*96]=omgrfn4_r2_tstart(source_id,t) word[879300]=0 omgrfn4_r2_next(source_id) return 0 }
    state bad { return omgrfn4_l2_reject() }
}"""
    return source[:match.start()] + lean + source[end:]


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


def before(source: str, marker: str) -> str:
    index = source.find(marker)
    if index < 0 or source.find(marker, index + 1) >= 0:
        raise ValueError(f"non-unique marker: {marker}")
    return source[:index]


def between(source: str, first: str, second: str) -> str:
    start = source.find(first)
    end = source.find(second)
    if start < 0 or end <= start:
        raise ValueError(f"bad marker interval: {first} .. {second}")
    return source[start:end]


def procedure(source: str, name: str) -> str:
    _, bodies = procedures(source)
    if name not in bodies:
        raise ValueError(f"missing procedure {name}")
    return bodies[name]


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


def write_checked(output: Path, name: str, source: str) -> str:
    proc_count, max_locals = shape(source)
    if proc_count > 128 or max_locals > 32:
        raise ValueError(f"{name} exceeds Beta shape: {proc_count}/128, {max_locals}/32")
    path = output / f"{name}.beta"
    path.write_text(source, encoding="ascii")
    return f"{name}\t{proc_count}\t{max_locals}\n"


def materialize(output: Path, outer: int = 8) -> None:
    if outer not in (8, 9, 10, 11):
        raise ValueError(f"unsupported OMGRFN outer version: {outer}")
    output.mkdir(parents=True, exist_ok=True)

    core = one_source_core(read("omgrfn4-source-witness-independent.beta"))
    declarations = read("omgrfn7-source-witness-independent.beta")

    r3 = replace_exact(
        read("omgrfn7-witness-ckir5-tables.beta"),
        "proc main() { return omgrfn7_r3_check() }",
        f"proc main() {{ return omgrfn{outer}_r3_check() }}",
    )
    r4_lowering = prune(
        core + "\n" + declarations + "\n" + read("omgrfn7-source-ckir5-lowering.beta")
        + f"\nproc main() {{ return omgrfn{outer}_r4_lowering_check() }}\n"
    )
    r4_result = prune(
        core + "\n" + lean_result_declarations(declarations) + "\n"
        + read("omgrfn7-source-lowering-meaning.beta")
        + f"\nproc main() {{ return omgrfn{outer}_r4_source_result_check() }}\n"
    )
    for forbidden in ("witness_byte", "ckir", "_elf_byte", "word[500088]", "word[500096]"):
        if forbidden in r4_result:
            raise ValueError(f"R4 source result gained forbidden reachability: {forbidden}")

    envelope = read("omgrfn7-component-envelope-r5.beta")
    artifact = read("ckir5-refinement-artifact.beta")
    result = read("ckir5-refinement-result.beta")
    elf = read("ckir5-refinement-elf.beta")
    r5_structure = envelope + "\n" + artifact
    artifact_core = (
        before(artifact, "proc ckir_constant_key_after")
        + between(artifact, "proc ckir_value_type", "proc ckir_initialize_call_graph")
        + between(artifact, "proc ckir_validate_operations", "proc ckir5_preserve_tables")
    )
    r5_result = envelope + artifact_core + before(result, "proc ckir5_refinement_artifact_check") + """
proc ckir_initialize_call_graph(){return 0}
proc ckir_record_call_edge(a,b){return 0}
proc ckir_validate_call_graph(){return 0}
proc main(){let s=omgrfn5_component_read() state a { to z when(s!=0) s=ckir_decode_header() to z when(s!=0) to bad when(ckir_count(7)!=0) s=ckir_validate_types_records() to z when(s!=0) s=ckir_validate_machines_blocks() to z when(s!=0) s=ckir_validate_operations() to z when(s!=0) s=ckir_validate_terminators_root() to z when(s!=0) s=ckir_assign_constructor_objects() to z when(s!=0) s=ckir_decode_header() to z when(s!=0) s=ckir_interpret_selected() to z } state bad{return 251} state z{return s}}
"""
    r5_elf = (
        envelope
        + before(artifact, "proc ckir_constant_key_after")
        + between(artifact, "proc ckir_value_type", "proc ckir_initialize_call_graph")
        + procedure(artifact, "ckir5_preserve_tables")
        + before(elf, "proc main()")
        + """proc main(){let s=omgrfn5_component_read() state a {to z when(s!=0) s=ckir_decode_header() to z when(s!=0) to bad when(ckir_count(7)!=0) s=ckir_validate_types_records() to z when(s!=0) s=ckir_validate_machines_blocks() to z when(s!=0) s=elf_assign_operation_types() to z when(s!=0) s=ckir5_preserve_tables() to z when(s!=0) s=ckir5_refinement_elf_check() to z} state bad{return 251} state z{return s}}
"""
    )

    rows = [
        write_checked(output, "r3", r3),
        write_checked(output, "r4-lowering", r4_lowering),
        write_checked(output, "r4-source-result", r4_result),
        write_checked(output, "r5-structure", r5_structure),
        write_checked(output, "r5-result", r5_result),
        write_checked(output, "r5-elf", r5_elf),
    ]
    (output / "manifest-r3-r5.tsv").write_text("".join(rows), encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("--outer", type=int, choices=(8, 9, 10, 11), default=8)
    args = parser.parse_args()
    materialize(args.output, args.outer)


if __name__ == "__main__":
    main()
