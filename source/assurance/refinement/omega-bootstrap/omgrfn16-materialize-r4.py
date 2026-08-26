#!/usr/bin/env python3
"""Materialize contract-complete persisted-Beta OMGRFN16 R4 owners."""

from __future__ import annotations

import argparse
import importlib.util
from pathlib import Path


HERE = Path(__file__).resolve().parent


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load {path.name}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BASE = load("omgrfn16_r4_base", HERE / "omgrfn8-materialize-r3-r5.py")
OLD = load("omgrfn16_r4_old", HERE / "omgrfn16-materialize-r3-r5.py")
R12 = load("omgrfn16_r4_r12", HERE / "omgrfn16-materialize-r1-r2.py")


def read(name: str) -> str:
    return (HERE / name).read_text(encoding="ascii")


def replace_proc(source: str, name: str, body: str) -> str:
    return OLD.replace_proc(source, name, body)


STRICT_FRAME_READER = OLD.R4_FRAME_READER.replace(
    "state flags_ok {\n",
    "state flags_ok {\n"
    "        to success when(word[500016]==1) to trap\n"
    "    }\n"
    "    state success { to bad when(word[500096]>255) to bad when(word[500096]!=word[500088]%256) to extents_check }\n"
    "    state trap { to bad when(word[500088]!=4294967295) to bad when(word[500096]!=4294967295) to extents_check }\n"
    "    state extents_check {\n",
)


def source_core() -> str:
    source = BASE.one_source_core(read("omgrfn4-source-witness-independent.beta"))
    source = replace_proc(source, "omgrfn4_r2_tokenize", R12.R2_TOKENIZE)
    return source


def lowering_source() -> str:
    inherited = source_core() + "\n" + read("omgrfn7-source-witness-independent.beta")
    inherited += "\n" + read("omgrfn7-source-ckir5-lowering.beta")
    inherited = replace_proc(inherited, "omgrfn5_r2_read_frame", STRICT_FRAME_READER)
    inherited = inherited.replace("omgrfn4_r2_tkind(source_id,t)!=256", "omgrfn4_r2_tkind(source_id,t)!=256")
    inherited = inherited.replace(
        "to bad when (r47_wbyte(6)!='3') to bad when (r47_wbyte(7)!=0)",
        "to bad when(r47_wbyte(6)!='7') to bad when(r47_wbyte(7)!=0)",
        1,
    )
    inherited = inherited.replace(
        "to bad when (r47_wbyte(8)!=3) to bad when (r47_wbyte(9)!=0)",
        "to bad when(r47_wbyte(8)!=7) to bad when(r47_wbyte(9)!=0)",
        1,
    )
    old = "state bparam { i=i+1 to bparams }"
    new = """state bparam { at=word[954304]+i*24 target=r47_wu32(at+4)
        word[893000+i*48]=target word[893008+i*48]=r47_wu32(at+8)
        word[893016+i*48]=r47_wu32(at+12) word[893024+i*48]=r47_wu32(at+16)
        word[893032+i*48]=r47_wu32(at+20) i=i+1 to bparams }"""
    if inherited.count(old) != 1:
        raise ValueError("OMGRFN16 R4 block-parameter witness anchor")
    inherited = inherited.replace(old, new)
    inherited = inherited.replace(
        "to bad when (word[950000]!=word[879008]+word[879344]+3+extra)",
        "to bad when(word[950000]!=word[954032])",
        1,
    )
    inherited = inherited.replace(
        "to bad when (word[950064]!=word[879040])\n"
        "        to bad when (word[950072]!=word[879048]) return 0",
        "to bad when(word[950064]<word[879040]) to bad when(word[950064]>word[879040]+1)\n"
        "        to bad when(word[950072]<word[879048]) to bad when(word[950072]>word[879048]+1)\n"
        "        to bad when(word[950064]-word[879040]!=word[950072]-word[879048]) return 0",
        1,
    )
    return BASE.prune(
        inherited + "\n" + read("omgrfn16-r4-expression.beta")
        + "\n" + read("omgrfn16-r4-lowering-owner.beta")
    )


def result_source() -> str:
    inherited = source_core() + "\n" + read("omgrfn7-source-witness-independent.beta")
    inherited = replace_proc(inherited, "omgrfn5_r2_read_frame", STRICT_FRAME_READER)
    return BASE.prune(
        inherited + "\n" + read("omgrfn16-r4-expression.beta")
        + "\n" + read("omgrfn16-r4-source-result-owner.beta")
    )


def materialize(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    rows = [
        BASE.write_checked(output, "r4-lowering", lowering_source()),
        BASE.write_checked(output, "r4-source-result", result_source()),
    ]
    (output / "manifest-r4.tsv").write_text("".join(rows), encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    materialize(args.output)


if __name__ == "__main__":
    main()
