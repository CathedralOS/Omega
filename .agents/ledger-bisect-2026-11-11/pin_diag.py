#!/usr/bin/env python3
import sys
from pathlib import Path

obs = Path(sys.argv[1]).read_text().splitlines()
witness = sys.argv[2]
for i, line in enumerate(obs):
    if line.startswith(witness + " "):
        if i + 1 < len(obs) and obs[i + 1].startswith("\t"):
            print(obs[i + 1][1:])
        break
