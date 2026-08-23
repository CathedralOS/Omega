#!/usr/bin/env python3
"""Compatibility entry point for Beta reference ownership/parser tests."""

from pathlib import Path
import runpy

_ROOT = Path(__file__).resolve().parents[2]
runpy.run_path(
    str(_ROOT / 'bootstrap/rungs/beta/reference/test_beta_parser.py'),
    run_name='__main__',
)
