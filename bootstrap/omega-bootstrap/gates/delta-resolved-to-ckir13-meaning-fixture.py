#!/usr/bin/env python3
"""Fixtures and independent observations for CKIR13 Rust-free lowering meaning."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def load_source_gate():
    path = HERE / "delta-resolved-to-ckir13-fixture.py"
    spec = importlib.util.spec_from_file_location("ckir13_meaning_source", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def prepare(output: Path) -> None:
    producer = load_source_gate()
    output.mkdir(parents=True, exist_ok=True)
    output.joinpath("canonical.omgc").write_bytes(
        producer.encode_source(producer.SUCCESS.read_text(encoding="ascii"))
    )
    output.joinpath("underflow.omgc").write_bytes(
        producer.encode_source(producer.UNDERFLOW.read_text(encoding="ascii"))
    )


def frame(output: Path) -> None:
    producer = load_source_gate()
    canonical_comp = output.joinpath("canonical.omgc").read_bytes()
    canonical_witness = output.joinpath("canonical.omgrsw5").read_bytes()
    underflow_comp = output.joinpath("underflow.omgc").read_bytes()
    underflow_witness = output.joinpath("underflow.omgrsw5").read_bytes()
    producer.inspect_witness(canonical_witness)
    producer.inspect_witness(underflow_witness)
    output.joinpath("canonical.omglowe").write_bytes(
        producer.pack_lowering(canonical_comp, canonical_witness)
    )
    output.joinpath("underflow.omglowe").write_bytes(
        producer.pack_lowering(underflow_comp, underflow_witness)
    )
    # A full-u32 range on an authored u8 witness row is structurally readable
    # but semantically inconsistent with the source graph.
    semantic_witness = producer.mutate_scalar_high(canonical_witness, 1, 0xFFFF_FFFF)
    output.joinpath("semantic-251.omglowe").write_bytes(
        producer.pack_lowering(canonical_comp, semantic_witness)
    )
    # The lowerer must reject the first byte beyond OMGCOMP's public component
    # ceiling before attempting to consume the absent payload.
    comp_size, witness_size = 267_281, 0
    total = producer.LOWER_HEADER.size + comp_size + witness_size
    output.joinpath("resource-252.omglowe").write_bytes(
        producer.LOWER_HEADER.pack(
            b"OMGLOWE\0", 14, 0, 0, producer.LOWER_HEADER.size,
            total, comp_size, witness_size, 5,
        )
    )


def check(output: Path) -> None:
    producer = load_source_gate()
    canonical = producer.ir13.decode(output.joinpath("canonical.expected").read_bytes())
    underflow = producer.ir13.decode(output.joinpath("underflow.expected").read_bytes())
    require(producer.ir13.selected_subtract_count(canonical) == 1,
            "canonical CKIR13 must contain one selected full-u32 subtraction")
    require(producer.ir13.selected_subtract_count(underflow) == 1,
            "underflow CKIR13 must contain one selected full-u32 subtraction")
    require(producer.ir13.interpret(canonical) == 70,
            "maximum-minus-near-maximum result must reach 70")
    try:
        producer.ir13.interpret(underflow)
    except producer.ir13.Ckir13Error:
        pass
    else:
        raise ValueError("zero-minus-one CKIR13 did not trap")


def main(arguments: list[str]) -> None:
    if len(arguments) != 2 or arguments[0] not in {"prepare", "frame", "check"}:
        raise SystemExit("usage: delta-resolved-to-ckir13-meaning-fixture.py prepare|frame|check DIR")
    command, output = arguments[0], Path(arguments[1])
    if command == "prepare":
        prepare(output)
    elif command == "frame":
        frame(output)
    else:
        check(output)


if __name__ == "__main__":
    main(sys.argv[1:])
