#!/usr/bin/env python3
"""Producer-backed OMGCOMP2/OMGRSW6 handoff fixture and inspection seam."""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
FIXTURE = HERE / "fixtures/omgrsw6-console-provider"
sys.path.insert(0, str(COMPILER))

import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation_v2 as compilation  # noqa: E402


def load_reference():
    path = HERE / "omgrsw6_provider_resolution_reference.py"
    spec = importlib.util.spec_from_file_location("omgrsw6_reference", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


REFERENCE = load_reference()


def source_contents() -> dict[str, bytes]:
    return {
        REFERENCE.LABELS[0]: (FIXTURE / "app-main.omg").read_bytes(),
        REFERENCE.LABELS[1]: (FIXTURE / "console.omg").read_bytes(),
        REFERENCE.LABELS[2]: (FIXTURE / "console-impl-linux-x64.omg").read_bytes(),
    }


def build(output: Path) -> None:
    contents = source_contents()
    source_bundle = bundle.encode([
        bundle.Entry(label, contents[label]) for label in REFERENCE.LABELS
    ])
    manifest = json.loads((FIXTURE / "manifest.json").read_text(encoding="utf-8"))
    envelope = compilation.encode_manifest(manifest, source_bundle)
    if envelope != REFERENCE.encode_envelope(contents):
        raise ValueError("OMGCOMP2 producer differs from independent V6 envelope")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(envelope)


def inspect(envelope: Path, witness: Path) -> None:
    view = REFERENCE.decode_witness(envelope.read_bytes(), witness.read_bytes())
    required = {"schema": "OMGRSW6", "bytes": 1064, "selected_machine": 0,
                "selection": None}
    for key, expected in required.items():
        if view.get(key) != expected:
            raise ValueError(f"OMGRSW6 inspection {key} drift")
    if view.get("candidate") != {
            "kind": "CompilerIntrinsic", "payload_bytes": 0, "target": 1}:
        raise ValueError("OMGRSW6 inspection candidate drift")
    if view.get("call_target") != {"id": 0, "kind": "requirement"}:
        raise ValueError("OMGRSW6 inspection call-target drift")
    print(json.dumps(view, sort_keys=True))


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    build_parser = subparsers.add_parser("build")
    build_parser.add_argument("output", type=Path)
    inspect_parser = subparsers.add_parser("inspect")
    inspect_parser.add_argument("envelope", type=Path)
    inspect_parser.add_argument("witness", type=Path)
    args = parser.parse_args()
    if args.command == "build":
        build(args.output)
    else:
        inspect(args.envelope, args.witness)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, compilation.CompilationError,
            REFERENCE.ReferenceError) as error:
        print(f"OMGRSW6 producer fixture: {error}", file=sys.stderr)
        raise SystemExit(251)
