#!/usr/bin/env python3
"""Re-run harness for every shard fixture with fixed parsing (serial)."""

import subprocess
import sys
from pathlib import Path

DIR = Path(__file__).resolve().parent
shard = int(sys.argv[1]) if len(sys.argv) > 1 else 1
subprocess.check_call([sys.executable, str(DIR / "run_shard.py"), str(shard), "rebuild"])
