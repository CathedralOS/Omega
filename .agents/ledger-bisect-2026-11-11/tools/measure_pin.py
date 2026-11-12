#!/usr/bin/env python3
"""Classify a fresh PIN corpus record file into measured.json and pin-witness json files."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bisect_test import norm_diag, parse_records  # noqa: E402

CLUSTERS = {
    "pass/targets/private_target_supplies_remain_disclosed": "c01",
    "pass/targets/retained_target_provider_family_exit": "c01",
    "pass/targets/retained_target_family_bodies": "c02",
    "pass/borrows/runtime_stored_reference_case_residual_retirement_exit": "c03",
    "run/borrows/terminal_case_completed_observation": "c04",
    "pass/traits/prepared_callable_nested_borrow_paths_exit": "c05",
    "pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit": "c06",
    "pass/facts/case_payload_exclusive_loan_through_other_machine_exit": "c07",
    "fail/contracts/target_family_recursive_guarantee": "",
    "fail/contracts/target_family_unshared_result": "",
    "fail/targets/sibling_body_type_error_rejected": "",
    "fail/targets/unrealized_conformance_rejected": "",
    "fail/borrows/aliased_forwarded_placed_views": "",
}

RANK = {"incomplete": 0, "rejected": 0, "checked": 1, "built": 2}


def ledger_row(text: str, fixture: str) -> dict | None:
    for line in text.splitlines():
        if not line.startswith(fixture + " "):
            continue
        parts = line.split()
        status = parts[1]
        regressed = None
        for tok in parts:
            if tok.startswith("regressed-from:"):
                regressed = tok.split(":", 1)[1]
        return {"ledger_status": status, "expected_status": regressed or status}
    return None


def main() -> int:
    records_path, ledger_path, out_dir = sys.argv[1:]
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    observed = parse_records(Path(records_path).read_text())
    ledger = Path(ledger_path).read_text()
    measured = []
    for fixture in CLUSTERS:
        obs = observed.get(fixture)
        row = ledger_row(ledger, fixture)
        if obs is None or row is None:
            measured.append(
                {
                    "fixture": fixture,
                    "cls": "missing",
                    "diag": "",
                    "cluster": CLUSTERS[fixture],
                }
            )
            continue
        diag = (obs.get("diagnostics") or [""])[0]
        tier = fixture.split("/")[0]
        expected = row["expected_status"]
        if tier == "fail":
            good = obs["status"] in ("rejected", "incomplete")
        else:
            good = RANK.get(obs["status"], -1) >= RANK.get(expected, -1)
            if obs.get("expected_satisfied") is False:
                good = False
        cls = "fixed" if good else "broken"
        measured.append(
            {
                "fixture": fixture,
                "cls": cls,
                "diag": diag,
                "status": obs["status"],
                "cluster": CLUSTERS[fixture],
                "expected_status": expected,
            }
        )
        if cls == "broken":
            slug = fixture.replace("/", "_")
            payload = {
                "witness": fixture,
                "tier": tier,
                "expected_status": expected,
                "ledger_status": row["ledger_status"],
                "pin_status": obs["status"],
                "pin_first_diagnostic": diag,
                "pin_diag_norm": norm_diag(diag),
            }
            (out / f"{slug}.json").write_text(json.dumps(payload, indent=2))
    Path(os.environ.get("MEASURED_OUT", "/tmp/ledger-bisect-part27/measured.json")).write_text(
        json.dumps(measured, indent=2)
    )
    print(json.dumps({m["fixture"]: m["cls"] for m in measured}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
