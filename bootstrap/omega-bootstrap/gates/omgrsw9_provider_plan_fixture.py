#!/usr/bin/env python3
"""Producer-backed OMGCOMP3/OMGRSW9 provider-plan fixture seam."""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
FIXTURE = HERE / "fixtures" / "omgcomp3-console-provider-plan"
sys.path.insert(0, str(COMPILER))

import omega_bootstrap_compilation_v3 as compilation  # noqa: E402


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


COMPILATION_FIXTURE = load_module(
    "omgcomp3_build_source_fixture", HERE / "omgcomp3_build_source_fixture.py"
)
REFERENCE = load_module(
    "omgrsw9_provider_plan_reference", HERE / "omgrsw9_provider_plan_reference.py"
)


def build(output: Path) -> None:
    bundle = COMPILATION_FIXTURE.build_bundle()
    manifest = json.loads((FIXTURE / "manifest.json").read_text(encoding="utf-8"))
    envelope = compilation.encode_manifest(manifest, bundle)
    if envelope != COMPILATION_FIXTURE.reference_encode(bundle):
        raise ValueError("OMGCOMP3 producer differs from its independent fixture bytes")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(envelope)


def inspect(envelope: Path, witness: Path) -> None:
    view = REFERENCE.decode_witness(envelope.read_bytes(), witness.read_bytes())
    required = {
        "schema": "OMGRSW9",
        "bytes": REFERENCE.WITNESS_BYTES,
        "selected_plan": 0,
        "selected_trait": 0,
        "selected_provider": 0,
    }
    for key, expected in required.items():
        if view.get(key) != expected:
            raise ValueError(f"OMGRSW9 inspection {key} drift")
    print(json.dumps(view, sort_keys=True))


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    build_parser = subparsers.add_parser("build")
    build_parser.add_argument("output", type=Path)
    inspect_parser = subparsers.add_parser("inspect")
    inspect_parser.add_argument("envelope", type=Path)
    inspect_parser.add_argument("witness", type=Path)
    arguments = parser.parse_args()
    if arguments.command == "build":
        build(arguments.output)
    else:
        inspect(arguments.envelope, arguments.witness)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, compilation.CompilationError,
            REFERENCE.ReferenceError) as error:
        print(f"OMGRSW9 producer fixture: {error}", file=sys.stderr)
        raise SystemExit(251)
