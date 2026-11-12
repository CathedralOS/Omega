#!/usr/bin/env python3
"""Freeze PIN ledger row and PIN observation for one witness bisect."""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

PIN = os.environ.get("BISECT_PIN", "51e8c45c793007f211d0d9baee7de16767172444")
REF_ROOT = Path(os.environ.get("BISECT_PIN_REF", "/tmp/ledger-bisect-p6-ref"))


def pin_ledger_text(repo: Path) -> str:
    return subprocess.check_output(
        ["git", "show", f"{PIN}:tests/omega/corpus_outcomes.txt"],
        cwd=repo,
        text=True,
    )


def expected_status(ledger_line: str, tier: str) -> str:
    parts = ledger_line.split()
    if tier == "fail":
        return "rejected" if "incomplete" not in parts else "incomplete"
    for tok in parts[3:]:
        if tok.startswith("regressed-from:"):
            base = tok.split(":", 1)[1].split("+")[0]
            if base in ("checked", "built", "rejected", "incomplete"):
                return base
    if len(parts) > 1 and parts[1] in ("checked", "built", "rejected", "incomplete"):
        return parts[1]
    return "checked" if tier == "pass" else "rejected"


def pin_observation(repo: Path, witness: str) -> dict:
    obs_path = repo / ".agents/ledger-bisect-2026-11-11/observed_pin.txt"
    if not obs_path.is_file():
        raise SystemExit(f"missing {obs_path}")
    rec = parse_records(obs_path.read_text(encoding="utf-8"))
    if witness not in rec:
        raise SystemExit(f"{witness} missing from observed_pin.txt")
    diag = (rec[witness]["diagnostics"] or [""])[0]
    return {
        "fixture": witness,
        "status": rec[witness]["status"],
        "first_diagnostic": diag,
        "diag_norm": norm_diag(diag),
    }


def main() -> int:
    witness = sys.argv[1]
    repo = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    ledger_text = pin_ledger_text(repo)
    ledger_line = next((ln for ln in ledger_text.splitlines() if ln.startswith(witness + " ")), "")
    if not ledger_line:
        raise SystemExit(f"{witness} missing from PIN ledger snapshot")
    tier = witness.split("/")[0]
    pin_obs = pin_observation(repo, witness)
    payload = {
        "pin": PIN,
        "witness": witness,
        "tier": tier,
        "ledger_line": ledger_line.strip(),
        "expected_status": expected_status(ledger_line, tier),
        "pin_broken_status": pin_obs["status"],
        "pin_first_diagnostic": pin_obs["first_diagnostic"],
        "pin_diag_norm": pin_obs["diag_norm"],
    }
    REF_ROOT.mkdir(parents=True, exist_ok=True)
    (REF_ROOT / "corpus_outcomes.txt").write_text(ledger_text, encoding="utf-8")
    safe = witness.replace("/", "__")
    (REF_ROOT / f"{safe}.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps({"ref_root": str(REF_ROOT), **payload}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
