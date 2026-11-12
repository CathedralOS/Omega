#!/usr/bin/env python3
"""Parse report-branch corpus batch outputs (full records + summary lines)."""
from __future__ import annotations

import re
import sys
from pathlib import Path


def parse_full(text: str) -> dict[str, dict]:
    records: dict[str, dict] = {}
    current: str | None = None
    for line in text.splitlines():
        if line.startswith("\t") and current:
            records[current]["diagnostics"].append(line[1:].strip())
            continue
        if not line.strip():
            continue
        if line.startswith("~ "):
            m = re.match(r"~ (\S+):", line)
            if m:
                fix = m.group(1)
                records.setdefault(
                    fix,
                    {"fixture": fix, "status": "summary", "diagnostics": [], "summary": line},
                )
            continue
        parts = line.split()
        if len(parts) < 3 or not parts[2].endswith("ms"):
            continue
        fix, status = parts[0], parts[1]
        records[fix] = {
            "fixture": fix,
            "status": status,
            "diagnostics": [],
            "summary": None,
        }
        current = fix
    return records


def main() -> int:
    merged: dict[str, dict] = {}
    for p in sorted(Path(sys.argv[1]).glob("batch_*.txt")):
        chunk = parse_full(p.read_text())
        for k, v in chunk.items():
            if k not in merged or v.get("status") != "summary":
                merged[k] = v
    out = Path(sys.argv[2])
    lines: list[str] = []
    for fix in sorted(merged):
        r = merged[fix]
        if r["status"] == "summary":
            continue
        lines.append(f"{fix} {r['status']} 0ms")
        for d in r["diagnostics"]:
            lines.append(f"\t{d}")
    out.write_text("\n".join(lines) + ("\n" if lines else ""))
    print(f"fixtures={len([x for x in merged.values() if x['status']!='summary'])} out={out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
