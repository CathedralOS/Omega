#!/usr/bin/env python3
"""Write PIN-fixed ledger + per-witness observation for bisect."""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
from classify_corpus import norm_diag, parse_records  # noqa: E402


def ledger_line(fixture: str, ledger_text: str) -> str | None:
    for line in ledger_text.splitlines():
        if line.startswith(fixture + " "):
            return line
    return None


def expected_status(line: str | None, tier: str) -> str | None:
    if not line:
        return "rejected" if tier == "fail" else "checked"
    parts = line.split()
    if len(parts) < 2:
        return None
    for tok in parts[2:]:
        if tok.startswith("regressed-from:"):
            return tok.split(":", 1)[1]
    return parts[1]


def main() -> int:
    witnesses = sys.argv[1:]
    measured = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("/tmp/slice_2_pin_measure.txt")
    ledger_src = REPO / "tests/omega/corpus_outcomes.txt"
    ledger_dst = Path("/tmp/bisect_pin_corpus_outcomes.txt")
    shutil.copy2(ledger_src, ledger_dst)
    ledger_text = ledger_dst.read_text()
    obs = parse_records(measured.read_text()) if measured.exists() else {}
    meta_dir = Path("/tmp/bisect_pin_meta")
    meta_dir.mkdir(exist_ok=True)
    for w in witnesses:
        rec = obs.get(w, {})
        diag = (rec.get("diagnostics") or [""])[0]
        tier = w.split("/")[0]
        ll = ledger_line(w, ledger_text)
        exp = expected_status(ll, tier)
        snap = {
            "witness": w,
            "pin_commit": PIN,
            "observed_status": rec.get("status", ""),
            "observed_diagnostic": diag,
            "observed_diag_norm": norm_diag(diag),
            "expected_status": exp,
            "ledger_line": ll,
        }
        safe = w.replace("/", "_")
        (meta_dir / f"{safe}.json").write_text(json.dumps(snap, indent=2))
        print(w, snap["observed_status"], "->", exp, diag[:60])
    (meta_dir / "manifest.json").write_text(json.dumps({"witnesses": witnesses, "pin": PIN}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
