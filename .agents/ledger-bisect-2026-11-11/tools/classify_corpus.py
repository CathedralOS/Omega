#!/usr/bin/env python3
"""Classify regressed sweep fixtures vs golden ledger."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
GOLDEN = REPO / "tests/omega/corpus_outcomes.txt"


def parse_records(text: str) -> dict[str, dict]:
    records: dict[str, dict] = {}
    current_key: str | None = None
    for line in text.splitlines():
        if line.startswith("\t") and current_key:
            records[current_key]["diagnostics"].append(line[1:].strip())
            continue
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 3 or not parts[2].endswith("ms"):
            continue
        fixture, status = parts[0], parts[1]
        tier = fixture.split("/")[0]
        flags = [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]
        expected = None
        if "expected" in flags:
            expected = True
        elif "unexpected" in flags:
            expected = False
        records[fixture] = {
            "fixture": fixture,
            "tier": tier,
            "status": status,
            "expected_satisfied": expected,
            "diagnostics": [],
        }
        current_key = fixture
    return records


def is_worse_than_ledger(prior: dict, observed: dict) -> bool:
    tier = observed["tier"]
    is_fail = tier == "fail"
    if is_fail:
        return prior["status"] in ("rejected", "incomplete") and observed["status"] not in (
            "rejected",
            "incomplete",
        )
    rank = {"built": 2, "checked": 1}

    def r(s: str) -> int:
        return rank.get(s, 0)

    status_regressed = r(observed["status"]) < r(prior["status"])
    expected_regressed = (
        prior.get("expected_satisfied") is True and observed.get("expected_satisfied") is False
    )
    return status_regressed or expected_regressed


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def ledger_row(fixture: str) -> dict | None:
    for line in GOLDEN.read_text().splitlines():
        if line.startswith(fixture + " "):
            parts = line.split()
            return {"status": parts[1]}
    return None


def classify_fixture(fixture: str, obs: dict) -> tuple[str, str]:
    """Return (class, first_diagnostic). class in fixed|broken|host_only|missing"""
    if obs.get("status") == "timeout":
        return "host_only", ""
    prior = ledger_row(fixture)
    if prior is None:
        tier = obs["tier"]
        if tier == "fail" and obs["status"] not in ("rejected", "incomplete"):
            diag = (obs.get("diagnostics") or [""])[0]
            return "broken", diag
        if tier != "fail" and obs["status"] in ("checked", "built"):
            return "fixed", ""
        if tier != "fail":
            diag = (obs.get("diagnostics") or [""])[0]
            return "broken", diag
        return "fixed", ""
    prior_full = parse_records(GOLDEN.read_text()).get(fixture, prior)
    if not is_worse_than_ledger(prior_full, obs):
        return "fixed", ""
    diag = (obs.get("diagnostics") or [""])[0]
    return "broken", diag


def classify_all(observed_path: Path, regressed_path: Path) -> dict:
    golden = parse_records(GOLDEN.read_text())
    observed = parse_records(observed_path.read_text())
    regressed = regressed_path.read_text().splitlines()
    fixed, broken, host_only, missing = [], [], [], []
    broken_detail: dict[str, dict] = {}
    for fix in regressed:
        fix = fix.strip()
        if not fix:
            continue
        if fix not in observed:
            missing.append(fix)
            continue
        obs = observed[fix]
        if obs["status"] == "timeout":
            host_only.append(fix)
            continue
        if fix not in golden:
            if obs["tier"] == "fail" and obs["status"] not in ("rejected", "incomplete"):
                broken.append(fix)
            elif obs["tier"] != "fail" and obs["status"] in ("checked", "built"):
                fixed.append(fix)
            elif obs["tier"] != "fail":
                broken.append(fix)
            else:
                fixed.append(fix)
            continue
        prior = golden[fix]
        if not is_worse_than_ledger(prior, obs):
            fixed.append(fix)
        else:
            broken.append(fix)
            diag = (obs["diagnostics"] or prior.get("diagnostics") or [""])[0]
            broken_detail[fix] = {
                "diag": diag,
                "diag_norm": norm_diag(diag),
                "main_status": obs["status"],
                "ledger_status": prior["status"],
            }
    return {
        "counts": {
            "fixed": len(fixed),
            "broken": len(broken),
            "host_only": len(host_only),
            "missing": len(missing),
            "total": len([f for f in regressed if f.strip()]),
        },
        "fixed": fixed,
        "broken": broken,
        "host_only": host_only,
        "missing": missing,
        "broken_detail": broken_detail,
    }


if __name__ == "__main__":
    data = classify_all(Path(sys.argv[1]), Path(sys.argv[2]))
    print(json.dumps(data, indent=2))
