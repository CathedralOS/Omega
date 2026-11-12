#!/usr/bin/env python3
"""Merge one bisect JSON line into state.json by witness."""
from __future__ import annotations

import json
import sys
from pathlib import Path

STATE = Path(__file__).resolve().parents[1] / "state.json"


def main() -> int:
    raw = sys.stdin.read()
    line = next((ln.strip() for ln in raw.splitlines() if ln.strip().startswith("{")), "")
    if not line:
        print("no json on stdin", file=sys.stderr)
        return 1
    result = json.loads(line)
    state = json.loads(STATE.read_text())
    witness = result["witness"]
    for cl in state["pin_clusters"]:
        if cl["witness"] == witness:
            if "error" in result:
                cl["bisect"] = {"status": "propagated", "reason": result["error"]}
            else:
                fb = result.get("first_bad", "")
                cl["bisect"] = {
                    "status": "bisected",
                    "good": result.get("good", ""),
                    "first_bad": fb,
                    "pr": result.get("pr", ""),
                    "mechanism": result.get("mechanism", ""),
                    "flaky_good": bool(int(result.get("flaky_good", 0))),
                    "flaky_bad": bool(int(result.get("flaky_bad", 0))),
                }
            break
    STATE.write_text(json.dumps(state, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
