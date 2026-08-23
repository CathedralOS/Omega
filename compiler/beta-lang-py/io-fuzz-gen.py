#!/usr/bin/env python3
"""Compatibility entry point for the Beta I/O fuzz generator."""

from pathlib import Path
import runpy

_ROOT = Path(__file__).resolve().parents[2]
runpy.run_path(
    str(_ROOT / 'bootstrap/rungs/beta/reference/io-fuzz-gen.py'),
    run_name='__main__',
)
