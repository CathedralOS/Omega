#!/usr/bin/env python3
"""Compatibility entry point for the Beta symbolic-loop soundness gate."""

from pathlib import Path
import runpy

_ROOT = Path(__file__).resolve().parents[2]
runpy.run_path(
    str(_ROOT / 'bootstrap/assurance/refinement/beta/symbolic_loop_check.py'),
    run_name='__main__',
)
