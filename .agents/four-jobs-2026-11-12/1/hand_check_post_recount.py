#!/usr/bin/env python3
"""Hand-check soundness survivors and former tally-disagree partial reclassifications."""

from __future__ import annotations

import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HARNESS = REPO / "target/debug/examples/route_one_comparison"
OUT = Path(__file__).resolve().parent / "shard-2-hand-check-post-recount.md"

FIXTURES = [
    (
        "soundness-candidate",
        "tests/omega/fail/build/uefi_program_entry_missing_storage_roots/main.omg",
        "Route judged compilation `accepted` (0 obligations); current rejects UEFI entry/storage-root contract — defect outside lowered entry contract (documented open gate item).",
    ),
    (
        "was-tally-disagree-now-partial",
        "tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg",
        "Helper `machine:main` accepted; defect machine `invalid` is `outside` — partial, not program accept.",
    ),
    (
        "was-tally-disagree-now-partial",
        "tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg",
        "Defect machines `change`/`invalid` outside (mutable parameter); `main` accepted — partial.",
    ),
    (
        "was-tally-disagree-now-partial",
        "tests/omega/fail/asm/borrow_read_after_backward_write/main.omg",
        "Defect-related machines outside; `good`/`bad` accepted — partial.",
    ),
    (
        "was-tally-disagree-now-partial",
        "tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg",
        "Multiple machines outside; `done`/`bad` accepted — partial.",
    ),
]


def run(fixture: str) -> str:
    cmd = [str(HARNESS)]
    if "/fail/" in fixture:
        cmd.append("--every-machine")
    cmd.append(fixture)
    proc = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=300,
        stdin=subprocess.DEVNULL,
        cwd=str(REPO),
    )
    lines = []
    for line in (proc.stdout + proc.stderr).splitlines():
        if line.startswith(("program\t", "current-diagnostic\t")):
            lines.append(line[:600])
    return "\n".join(lines[:10]) if lines else "(no output)"


def main() -> None:
    parts = [
        "# Post-recount hand-check (5 fixtures)",
        "",
        "After the `partial` rule, only **one** row remains "
        "`route_verdict=accepted` with `current_verdict=rejected` (soundness watch). "
        "Four additional rows are former tally-disagree cases now classified `partial`.",
        "",
    ]
    for label, fixture, note in FIXTURES:
        parts.append(f"## `{fixture}` ({label})\n")
        parts.append(f"**Verdict:** {note}\n")
        parts.append("```\n" + run(fixture) + "\n```\n")
    OUT.write_text("\n".join(parts))
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
