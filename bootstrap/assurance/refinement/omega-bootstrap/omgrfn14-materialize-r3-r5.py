#!/usr/bin/env python3
"""Pure OMGRFN14 R3-R5 persisted-Beta source materializer."""

from __future__ import annotations

import argparse
from pathlib import Path

import omgrfn14_checker_model as model


def main_source(body: str, *, source_only: bool = False) -> str:
    return (model.SOURCE_COMMON if source_only else model.COMMON) + body


def materialize(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    sources = model.source_model()
    source_code = model.source_profile_code(sources)
    ckir_code = model.profile_selector() + model.exact_component(
        "expected_ckir", "ckirbyte", 700040, model.CKIR, 7_100_000,
    )
    elf_code = model.exact_component(
        "expected_elf", "elfbyte", 700056, model.ELF, 7_200_000,
    )

    r3 = main_source(model.WITNESS_CHECK + ckir_code + """
proc main() {
    let status=omgrfn14_read_frame() let profile=0
    state framed { to done when(status!=0) status=omgrfn14_witness_check() to witnessed }
    state witnessed { to done when(status!=0) profile=omgrfn14_profile() to selected }
    state selected { to bad when(profile==0) status=expected_ckir_check(profile) to done }
    state bad { return 251 }
    state done { return status }
}
""")
    r4_lowering = main_source(source_code + ckir_code + """
proc main() {
    let status=omgrfn14_read_frame() let source_profile=0 let ckir_profile=0
    state framed { to done when(status!=0) source_profile=omgrfn14_source_profile() ckir_profile=omgrfn14_profile() to profiles }
    state profiles { to bad when(source_profile==0) to bad when(source_profile!=ckir_profile) status=expected_ckir_check(ckir_profile) to done }
    state bad { return 251 }
    state done { return status }
}
""")
    r4_result = main_source(source_code + """
proc main() {
    let status=omgrfn14_read_frame() let profile=0
    state framed { to done when(status!=0) profile=omgrfn14_source_profile() to source }
    state source { to bad when(profile==0) return 0 }
    state bad { return 251 }
    state done { return status }
}
""", source_only=True)
    r5_structure = main_source(ckir_code + """
proc main() {
    let status=omgrfn14_read_frame() let profile=0
    state framed { to done when(status!=0) profile=omgrfn14_profile() to selected }
    state selected { to bad when(profile==0) status=expected_ckir_check(profile) to done }
    state bad { return 251 }
    state done { return status }
}
""")
    r5_result = main_source(ckir_code + """
proc main() {
    let status=omgrfn14_read_frame() let profile=0
    state framed { to done when(status!=0) profile=omgrfn14_profile() to selected }
    state selected { to bad when(profile==0) status=expected_ckir_check(profile) to checked }
    state checked { to done when(status!=0) to bad when(word[700064]!=70) return 0 }
    state bad { return 251 }
    state done { return status }
}
""")
    r5_elf = main_source(ckir_code + elf_code + """
proc main() {
    let status=omgrfn14_read_frame() let profile=0
    state framed { to done when(status!=0) profile=omgrfn14_profile() to selected }
    state selected { to bad when(profile==0) status=expected_ckir_check(profile) to ckir }
    state ckir { to done when(status!=0) status=expected_elf_check(profile) to done }
    state bad { return 251 }
    state done { return status }
}
""")

    rows = [
        model.write_checked(output, "r3", r3),
        model.write_checked(output, "r4-lowering", r4_lowering),
        model.write_checked(output, "r4-source-result", r4_result),
        model.write_checked(output, "r5-structure", r5_structure),
        model.write_checked(output, "r5-result", r5_result),
        model.write_checked(output, "r5-elf", r5_elf),
    ]
    (output / "manifest-r3-r5.tsv").write_text("".join(rows), encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    materialize(args.output)


if __name__ == "__main__":
    main()
