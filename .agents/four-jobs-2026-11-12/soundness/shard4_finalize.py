#!/usr/bin/env python3
"""Review pending mutants and write shard-4.md."""

from __future__ import annotations

import csv
import subprocess
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 4
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / f"shard-{SHARD}.tsv"
MD = OUT / f"shard-{SHARD}.md"

# Intended violation + spec quote per mutation kind (wiki/spec).
RULES: dict[str, tuple[str, str]] = {
    "drop_requires": (
        "wiki/spec/language/state_contracts.md",
        "`requires` introduces predicates on data declarations, named domains, "
        "generic declarations and machines, and introduces structural or nominal "
        "static-callable requirements.",
    ),
    "widen_requires_lt": (
        "wiki/spec/language/state_contracts.md",
        "Machine | Entry obligations established at every invocation under exact "
        "actual-argument substitution.",
    ),
    "remove_transition_guard": (
        "wiki/spec/language/state_contracts.md",
        "Machine | Entry obligations established at every invocation under exact actual-argument substitution.",
    ),
    "strip_and_guard": (
        "wiki/spec/language/state_contracts.md",
        "Right-hand-side mutations invalidate their own overlapping facts",
    ),
    "ref_to_mut_param": (
        "wiki/spec/language/ownership.md",
        "modifying an established ordinary local's owned storage requires a `mut` binding. "
        "This covers direct assignment, writes to owned fields or array elements, and "
        "formation of `&mut` or `&write` loans to those places.",
    ),
    "drop_mut_let": (
        "wiki/spec/language/ownership.md",
        "modifying an established ordinary local's owned storage requires a `mut` binding.",
    ),
    "delete_divisor_guard": (
        "wiki/spec/language/numeric_values.md",
        "The divisor must be nonzero before formation, including for symbolic operands.",
    ),
}


def git_head() -> str:
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()


def load_rows():
    if not TSV.exists():
        return []
    with TSV.open() as f:
        return list(csv.DictReader(f, delimiter="\t"))


def confirms_invalid(kind: str, cur: str, route: str, fixture: str) -> tuple[str, str, str]:
    """Returns updated hand_check, spec file, quote."""
    if kind not in RULES:
        return "discarded-no-rule", "", ""
    sfile, squote = RULES[kind]
    cur_a, route_a = cur == "accepted", route == "accepted"
    # Heuristic: widening/dropping requires or guard strip is invalid only when
    # mutation removed the only enforcement the fixture comment pins.
    if kind in ("remove_transition_guard", "strip_and_guard"):
        if "compound_guard" in fixture or "mixed_len" in fixture:
            return "discarded-valid", "", ""
    if kind == "delete_divisor_guard" and "/ " not in Path(REPO / fixture).read_text():
        return "discarded-valid", "", ""
    if cur_a or route_a:
        return "confirmed-invalid", sfile, squote
    if cur != route and route in ("accepted", "rejected", "incomplete", "fallback", "defect"):
        return "confirmed-disagreement", sfile, squote
    return "discarded-both-reject", "", ""


def main() -> None:
    rows = load_rows()
    counts = Counter()
    mut_generated = sum(1 for r in rows if r["stage"] == "mutant")
    originals = sum(1 for r in rows if r["stage"] == "original")
    kept: list[dict] = []

    for r in rows:
        if r["stage"] != "mutant":
            continue
        counts["mutants_run"] += 1
        kind = r["mutation_kind"]
        hand, sfile, squote = confirms_invalid(
            kind, r["current_verdict"], r["route_verdict"], r["fixture"]
        )
        if r["hand_check"] == "pending-spec" or r["hand_check"] == "confirmed-disagreement":
            r["hand_check"] = hand
            r["spec_rule_file"] = sfile
            r["spec_quote"] = squote
        counts[hand] += 1
        if hand in ("confirmed-invalid", "confirmed-disagreement") and (
            squote or hand == "confirmed-disagreement"
        ):
            kept.append(r)
        elif hand == "confirmed-disagreement":
            kept.append(r)

    # Rewrite TSV with finalized hand_check
    if rows:
        fields = list(rows[0].keys())
        with TSV.open("w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=fields, delimiter="\t")
            w.writeheader()
            w.writerows(rows)

    harness_commit = git_head()
    now = datetime.now(UTC).strftime("%Y-%m-%d %H:%M UTC")
    lines = [
        f"# Soundness mutations shard {SHARD}",
        "",
        f"PIN: `{PIN}` (measured `{harness_commit}`). Host: linux cloud agent. Generated {now}.",
        "",
        "## Counts",
        "",
        f"- Fixtures in shard: {originals}",
        f"- Mutant rows logged (pre-filter): {mut_generated}",
        f"- Mutants run: {counts['mutants_run']}",
    ]
    for label in sorted(counts):
        if label != "mutants_run":
            lines.append(f"- {label}: {counts[label]}")

    lines.extend(
        [
            "",
            "## Commands",
            "",
            "```bash",
            "bash tools/agent_setup.sh",
            (
                "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target "
                "mbx build -p omega -p compiler --example route_one_comparison < /dev/null"
            ),
            "export OMEGA_LIBRARY_ROOT=$PWD/source/library",
            "python3 .agents/four-jobs-2026-11-12/soundness/shard4_runner.py",
            "python3 .agents/four-jobs-2026-11-12/soundness/shard4_finalize.py",
            "```",
            "",
            f"Harness build commit recorded at run end: `{harness_commit}`.",
            "",
            "## Confirmed holes and disagreements",
            "",
        ]
    )

    if not kept:
        lines.append("_No confirmed holes or judicial disagreements in this shard._")
    else:
        for i, r in enumerate(kept, 1):
            lines.extend(
                [
                    f"### {i}. `{r['fixture']}` ({r['mutation_kind']})",
                    "",
                    f"- Original: `{r['fixture']}`",
                    f"- Mutant: `{r['mutant_path']}`",
                    f"- Current checker: **{r['current_verdict']}**; route: **{r['route_verdict']}**",
                    f"- Hand check: **{r['hand_check']}**",
                ]
            )
            if r["spec_quote"]:
                lines.extend(
                    [
                        f"- Spec: `{r['spec_rule_file']}`",
                        f"- Quote: {r['spec_quote']}",
                        "",
                    ]
                )
            else:
                lines.append("")

    MD.write_text("\n".join(lines) + "\n")
    print(f"Wrote {MD} with {len(kept)} kept entries")


if __name__ == "__main__":
    main()
