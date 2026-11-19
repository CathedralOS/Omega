"""List second-segment observations: sub.py usage.json crate [module ...]."""
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    usage = json.load(source)
crate, modules = sys.argv[2], sys.argv[3:]
for item, users in sorted(usage["usage2"].get(crate, {}).items()):
    if modules and item.split("::")[0] not in modules:
        continue
    positions = sorted({usage["pos"].get(name, name) for name in users})
    print(f"{item} ({len(positions)}): {', '.join(positions)}")
