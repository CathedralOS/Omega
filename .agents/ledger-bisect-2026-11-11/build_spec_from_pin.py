#!/usr/bin/env python3
"""Build /tmp/ledger_bisect_part23_spec_<cluster_id>.json from pin_observed + ledger."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path


def parse_obs(obs_path: Path, witness: str) -> tuple[str, str]:
    lines = obs_path.read_text().splitlines()
    for i, line in enumerate(lines):
        if line.startswith(witness + " "):
            status = line.split()[1]
            diag = ""
            if i + 1 < len(lines) and lines[i + 1].startswith("\t"):
                diag = lines[i + 1][1:].strip()
            return status, diag
    raise SystemExit(f"missing witness in observation: {witness}")


def expected_status(ledger_path: Path, witness: str) -> str:
    for line in ledger_path.read_text().splitlines():
        if not line.startswith(witness + " "):
            continue
        status = line.split()[1]
        reg = next((t for t in line.split() if t.startswith("regressed-from:")), None)
        if reg:
            return reg.split(":", 1)[1].split("+")[0]
        tier = witness.split("/")[0]
        if tier == "fail":
            return "rejected"
        return status if status in ("checked", "built") else "checked"
    raise SystemExit(f"missing witness in ledger: {witness}")


def main() -> int:
    cluster_id, witness, obs, ledger = sys.argv[1:5]
    pin_status, pin_diag = parse_obs(Path(obs), witness)
    expected = expected_status(Path(ledger), witness)
    spec = {
        "cluster_id": cluster_id,
        "witness": witness,
        "pin_status": pin_status,
        "expected_good_status": expected,
    }
    if len(pin_diag) > 220:
        spec["pin_first_diagnostic_prefix"] = pin_diag[:200]
    elif pin_diag:
        spec["pin_first_diagnostic"] = pin_diag
    out = Path(f"/tmp/ledger_bisect_part23_spec_{cluster_id}.json")
    out.write_text(json.dumps(spec, indent=2) + "\n")
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
