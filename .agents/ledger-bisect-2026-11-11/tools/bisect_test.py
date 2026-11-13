#!/usr/bin/env python3
"""Classify one corpus record for a bisect witness.

Modes (BISECT_WITNESS_MODE):
  stop  — 0 ledger-expected outcome, 1 any other outcome that ran
  class — 1 current PIN diagnostic class, 0 any other outcome that ran

125 is only for a missing record or a run that did not finish (timeout).
A different rejection is exit 1 in stop mode and exit 0 in class mode.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

RANK = {"built": 2, "checked": 1}


def load_expect() -> dict:
    path = os.environ.get("BISECT_EXPECT_FILE")
    if not path:
        raise RuntimeError("BISECT_EXPECT_FILE not set")
    return json.loads(Path(path).read_text())


def write_reason(code: int, reason: str, detail: str = "") -> None:
    rf = os.environ.get("BISECT_REASON_FILE")
    if not rf:
        return
    Path(rf).write_text(f"exit={code}\t{reason}\t{detail}\n")


def ledger_match(obs: dict, expect: dict) -> bool:
    """0-side of the stop witness: the outcome the PIN ledger calls for.

    Fail rows match when they reject and every fragment in the PIN
    `expected.txt` appears in the diagnostics. The diagnostic text stored on
    an `unexpected` ledger row is the wrong reason, not the oracle.
    Pass and run rows match when they reach the ledger status, honoring
    `regressed-from:<status>`.
    """
    tier = expect["tier"]
    if tier == "fail":
        if obs.get("status") not in ("rejected", "incomplete"):
            return False
        fragments = expect.get("expected_fragments") or []
        if not fragments:
            return obs.get("status") == expect.get("ledger_status")
        blob = "\n".join(obs.get("diagnostics") or [])
        return all(fragment in blob for fragment in fragments)
    need = RANK.get(expect["good_status"], 0)
    got = RANK.get(obs.get("status") or "", 0)
    return got >= need and need > 0


def class_match(obs: dict, expect: dict) -> bool:
    diag0 = norm_diag((obs.get("diagnostics") or [""])[0])
    want = expect.get("pin_bad_diag_norm") or ""
    return bool(want) and diag0 == want


def classify_obs(obs: dict | None, expect: dict, mode: str) -> int:
    if obs is None:
        return 125
    if obs.get("status") in (None, "", "timeout"):
        return 125
    if mode == "stop":
        return 0 if ledger_match(obs, expect) else 1
    if mode == "class":
        return 1 if class_match(obs, expect) else 0
    raise RuntimeError(f"unknown mode {mode}")


def observe(text: str, fixture: str) -> dict | None:
    if not text.strip():
        return None
    recs = parse_records(text if text.endswith("\n") else text + "\n")
    return recs.get(fixture)


def main() -> int:
    fixture, rec_path = sys.argv[1], Path(sys.argv[2])
    mode = os.environ.get("BISECT_WITNESS_MODE", "stop")
    try:
        expect = load_expect()
    except Exception as exc:
        write_reason(125, "expect-load-fail", str(exc))
        return 125
    if fixture != expect["witness"]:
        write_reason(125, "witness-mismatch", fixture)
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    obs = observe(text, fixture)
    if obs is None:
        write_reason(125, "no-corpus-record", rec_path.as_posix())
        return 125
    code = classify_obs(obs, expect, mode)
    diag0 = norm_diag((obs.get("diagnostics") or [""])[0])
    if code == 125:
        write_reason(125, "did-not-run", f"status={obs.get('status')}")
    elif mode == "stop" and code == 0:
        write_reason(0, "ledger-match", f"status={obs.get('status')}")
    elif mode == "stop":
        write_reason(1, "not-ledger", f"status={obs.get('status')} diag={diag0[:120]}")
    elif code == 1:
        write_reason(1, "pin-class", f"status={obs.get('status')} diag={diag0[:120]}")
    else:
        write_reason(0, "other-class", f"status={obs.get('status')} diag={diag0[:120]}")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
