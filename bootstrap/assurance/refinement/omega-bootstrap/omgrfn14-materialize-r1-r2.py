#!/usr/bin/env python3
"""Pure OMGRFN14 R1/R2 persisted-Beta source materializer."""

from __future__ import annotations

import argparse
from pathlib import Path

import omgrfn14_checker_model as model


HERE = Path(__file__).resolve().parent


def materialize(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    r1 = (
        (HERE / "omgrfn4-frame-omgcomp-custody.beta").read_text(encoding="ascii")
        + "\n"
        + (HERE / "omgrfn5-frame-omgcomp-custody.beta").read_text(encoding="ascii")
        + "\nproc main() { return omgrfn14_layer1_check() }\n"
    )
    sources = model.source_model()
    r2 = (
        model.SOURCE_COMMON
        + model.WITNESS_CHECK
        + model.source_profile_code(sources)
        + """
proc main() {
    let status=omgrfn14_read_frame() let profile=0
    state framed { to done when(status!=0) status=omgrfn14_witness_check() to witnessed }
    state witnessed { to done when(status!=0) profile=omgrfn14_source_profile() to source }
    state source { to bad when(profile==0) return 0 }
    state bad { return 251 }
    state done { return status }
}
"""
    )
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
