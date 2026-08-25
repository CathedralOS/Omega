#!/usr/bin/env python3
"""Pure OMGRFN12 R3-R5 source materializer using the shared owner module."""
from __future__ import annotations
import argparse, importlib.util
from pathlib import Path
HERE=Path(__file__).resolve().parent
def main() -> None:
    parser=argparse.ArgumentParser(); parser.add_argument("output",type=Path); args=parser.parse_args()
    path=HERE/"omgrfn8-materialize-r3-r5.py"; spec=importlib.util.spec_from_file_location("omgrfn_shared_r3_r5",path)
    assert spec is not None and spec.loader is not None
    module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); module.materialize(args.output,12)
if __name__=="__main__": main()
