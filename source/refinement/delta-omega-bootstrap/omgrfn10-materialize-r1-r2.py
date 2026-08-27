#!/usr/bin/env python3
"""Pure OMGRFN10 R1/R2 source materializer using the shared V8-V10 module."""

from __future__ import annotations

import argparse
import importlib.util
from pathlib import Path


HERE = Path(__file__).resolve().parent


def shared_module():
    path = HERE / "omgrfn8-materialize-r1-r2.py"
    spec = importlib.util.spec_from_file_location("omgrfn_shared_r1_r2", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    shared_module().materialize(args.output, 10)


if __name__ == "__main__":
    main()
