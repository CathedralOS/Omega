#!/usr/bin/env python3
"""Pure OMGRFN11 R1-R2 source materializer using the shared owner module."""
from __future__ import annotations
import argparse, importlib.util
from pathlib import Path
HERE=Path(__file__).resolve().parent
def main() -> None:
    parser=argparse.ArgumentParser(); parser.add_argument("output",type=Path); args=parser.parse_args()
    path=HERE/"omgrfn8-materialize-r1-r2.py"; spec=importlib.util.spec_from_file_location("omgrfn_shared_r1_r2",path)
    assert spec is not None and spec.loader is not None
    module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); module.materialize(args.output,11)
if __name__=="__main__": main()
