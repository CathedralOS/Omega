#!/usr/bin/env python3
"""Compatibility entry point; build the shared atlas and report its plan page."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build import build_all

if __name__ == "__main__":
    print(build_all() / "plan.html")
