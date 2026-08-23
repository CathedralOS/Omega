#!/usr/bin/env python3
"""Compatibility entry point for Beta's canonical reference interpreter."""

from pathlib import Path
import sys

_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT))
from bootstrap.rungs.beta.reference.beta_interp import *  # noqa: F401,F403,E402

if __name__ == '__main__':
    main()
