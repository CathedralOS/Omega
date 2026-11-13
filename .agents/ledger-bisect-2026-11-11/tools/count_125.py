#!/usr/bin/env python3
"""Count exit-125 steps in a bisect rc log."""
import sys
from pathlib import Path

log = Path(sys.argv[1]).read_text() if len(sys.argv) > 1 else ""
n = 0
for line in log.splitlines():
    if " 125" in line or "stop=125" in line or "class=125" in line:
        n += 1
print(n)
