#!/usr/bin/env python3
"""Copy PIN ledger to /tmp and write per-witness bisect specs from PIN measurement."""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
ROOT = Path(__file__).resolve().parents[2]
OUT = Path("/tmp/ledger-bisect-pin")
MEASURE = ROOT / ".agents/ledger-bisect-2026-11-11/part-31-measure.txt"


def first_diag(text: str, fixture: str) -> str:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith(fixture + " "):
            for follow in lines[i + 1 :]:
                if follow.startswith("\t"):
                    return follow[1:].strip()
                if follow.strip():
                    break
    return ""


def pin_status(text: str, fixture: str) -> str:
    for line in text.splitlines():
        if line.startswith(fixture + " "):
            return line.split()[1]
    raise KeyError(fixture)


def good_status(ledger_line: str) -> str:
    m = re.search(r"regressed-from:([^\s@]+)", ledger_line)
    if m:
        return m.group(1).split("+")[0]
    return ledger_line.split()[1]


def ledger_line(corpus: str, fixture: str) -> str:
    for line in corpus.splitlines():
        if line.startswith(fixture + " "):
            return line
    raise KeyError(fixture)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    ledger_text = subprocess.check_output(
        ["git", "-C", str(ROOT), "show", f"{PIN}:tests/omega/corpus_outcomes.txt"],
        text=True,
    )
    (OUT / "corpus_outcomes.txt").write_text(ledger_text)
    measure = MEASURE.read_text()
    witnesses = [
        "pass/borrows/runtime_selected_reference_mutation_complement_exit",
        "pass/borrows/runtime_uniform_projected_reference_selection_exit",
        "pass/write_only/returned_destination_transport_exit",
        "pass/facts/computed_record_window_through_other_machine_exit",
        "pass/facts/computed_record_window_through_other_state_exit",
        "pass/facts/match_computed_root_through_other_state_exit",
        "pass/generics/generic_result_parameter_receipt",
    ]
    for w in witnesses:
        spec = {
            "witness": w,
            "pin_status": pin_status(measure, w),
            "pin_first_diag": first_diag(measure, w),
            "good_status": good_status(ledger_line(ledger_text, w)),
        }
        path = OUT / f"{w.replace('/', '_')}.json"
        path.write_text(json.dumps(spec, indent=2))
        print(w, spec)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
