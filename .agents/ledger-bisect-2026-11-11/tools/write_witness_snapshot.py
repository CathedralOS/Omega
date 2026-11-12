#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag  # noqa: E402


def first_diag(text: str, fixture: str) -> tuple[str, str]:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith(fixture + " "):
            status = line.split()[1]
            for j in range(i + 1, len(lines)):
                if lines[j].startswith("\t"):
                    d = lines[j][1:].strip()
                    return status, d
                if lines[j].strip():
                    break
            return status, ""
    raise SystemExit(f"missing fixture {fixture} in measure file")


def main() -> None:
    witness, measure_path, out_path = sys.argv[1], Path(sys.argv[2]), Path(sys.argv[3])
    status, diag = first_diag(measure_path.read_text(), witness)
    payload = {
        "witness": witness,
        "pin_status": status,
        "pin_diag": diag,
        "pin_diag_norm": norm_diag(diag),
    }
    out_path.write_text(json.dumps(payload, indent=2))


if __name__ == "__main__":
    main()
