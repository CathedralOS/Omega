"""Observe foundation crate references: modscan.py metadata.json output.json."""
import json
import sys
from scanlib import Workspace, scan, write_json

if __name__ == "__main__":
    with open(sys.argv[1], encoding="utf-8") as metadata:
        write_json(sys.argv[2], scan(Workspace(json.load(metadata)), "foundation"))
