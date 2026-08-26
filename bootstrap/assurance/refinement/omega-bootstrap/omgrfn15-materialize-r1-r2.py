#!/usr/bin/env python3
"""Pure OMGRFN15 R1/R2 persisted-Beta source materializer."""

from __future__ import annotations

import argparse
from pathlib import Path

import omgrfn15_checker_model as model


HERE = Path(__file__).resolve().parent


def r1_adapter() -> str:
    """Version-local extension; leave every older persisted source unchanged."""
    shared = (HERE / "omgrfn5-frame-omgcomp-custody.beta").read_text(encoding="ascii")
    old = "        to version14_magic when (omgrfn5_byte(6) == 'E')\n        to malformed"
    new = "        to version14_magic when (omgrfn5_byte(6) == 'E')\n        to version15_magic when (omgrfn5_byte(6) == 'F')\n        to malformed"
    if shared.count(old) != 1:
        raise ValueError("OMGRFN15 R1 magic anchor drift")
    shared = shared.replace(old, new)
    old = "    state version14_magic { word[500104] = 14  to magic7 }"
    new = old + "\n    state version15_magic { word[500104] = 15  to magic7 }"
    if shared.count(old) != 1:
        raise ValueError("OMGRFN15 R1 version anchor drift")
    return shared.replace(old, new) + """
proc omgrfn15_layer1_check() {
    let status=omgrfn5_layer1_check()
    state checked { to done when(status!=0) to exact when(word[500104]==15) return 251 }
    state exact { return 0 }
    state done { return status }
}
"""


def materialize(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    r1 = (
        (HERE / "omgrfn4-frame-omgcomp-custody.beta").read_text(encoding="ascii")
        + "\n" + r1_adapter()
        + "\nproc main() { return omgrfn15_layer1_check() }\n"
    )
    sources = model.source_model()
    source_code = model.source_profile_code(sources)
    witness_code = model.exact_component(
        "expected_witness", "witnessbyte", 700024, model.WITNESS, 7_050_000,
    )
    r2 = model.COMMON + source_code + witness_code + """
proc main() {
    let status=omgrfn15_read_frame() let source_profile=0 let witness_profile=0
    state framed { to done when(status!=0) source_profile=omgrfn15_source_profile() to source }
    state source { to bad when(source_profile==0) witness_profile=source_profile status=expected_witness_check(witness_profile) to done }
    state bad { return 251 }
    state done { return status }
}
"""
    rows = [
        model.write_checked(output, "r1", r1),
        model.write_checked(output, "r2", r2),
    ]
    (output / "manifest.tsv").write_text("".join(rows), encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    materialize(args.output)


if __name__ == "__main__":
    main()
