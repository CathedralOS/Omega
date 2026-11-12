#!/usr/bin/env python3
"""Concatenate batch record files into one observed corpus."""
from __future__ import annotations

import argparse
from pathlib import Path


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("out", type=Path)
    ap.add_argument("inputs", nargs="+", type=Path)
    args = ap.parse_args()
    parts: list[str] = []
    for p in args.inputs:
        if p.exists() and p.stat().st_size:
            parts.append(p.read_text())
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("".join(parts))
    print(f"wrote {args.out} ({args.out.stat().st_size} bytes) from {len(args.inputs)} inputs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
