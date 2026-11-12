#!/usr/bin/env python3
"""Capture harness snippets for agreement hand-check (shard notes)."""

from __future__ import annotations

import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HARNESS = REPO / "target/debug/examples/route_one_comparison"

SAMPLES = {
    "disagree_route_accept_current_reject": [
        "tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg",
        "tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg",
        "tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg",
        "tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg",
        "tests/omega/fail/asm/borrow_read_after_backward_write/main.omg",
    ],
    "disagree_route_reject_current_accept": [
        "tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg",
        "tests/omega/pass/arithmetic/runtime_boolean_divisor_state_exit/main.omg",
        "tests/omega/pass/effects/private_crashes_none_call_disproved_exit/main.omg",
        "tests/omega/pass/facts/bounded_scalar_sums_through_other_state_exit/main.omg",
        "tests/omega/pass/facts/match_join_multi_variant_bound_through_parameter_exit/main.omg",
    ],
}


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
        if line.startswith(("program\t", "fallback\t", "current-diagnostic\t")):
            lines.append(line[:500])
    return "\n".join(lines[:8]) if lines else "(no output)"


def main() -> None:
    out = []
    for group, fixtures in SAMPLES.items():
        out.append(f"## Hand-check group: {group}\n")
        for fx in fixtures:
            out.append(f"### `{fx}`\n")
            out.append("```\n" + run(fx) + "\n```\n")
    path = Path(__file__).resolve().parent / "shard-2-hand-check.md"
    path.write_text("\n".join(out))
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
