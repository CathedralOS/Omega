#!/usr/bin/env python3
"""Render part-31.md and part-31.tsv from measurement + bisect artifacts."""
from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
MEASURE = ROOT / ".agents/ledger-bisect-2026-11-11/part-31-measure.txt"
OUT_MD = ROOT / ".agents/ledger-bisect-2026-11-11/part-31.md"
OUT_TSV = ROOT / ".agents/ledger-bisect-2026-11-11/part-31.tsv"

CLUSTERS = [
    {
        "id": "c0",
        "report_idx": 31,
        "witness": "pass/borrows/runtime_selected_reference_mutation_complement_exit",
        "members": [
            "fail/borrows/conditional_reference_replacement_false_first_stale_index",
            "fail/borrows/conditional_reference_replacement_true_first_stale_index",
            "fail/slices/reborrowed_replaced_descriptor",
            "pass/borrows/runtime_selected_reference_mutation_complement_exit",
            "pass/borrows/runtime_uniform_projected_reference_selection_exit",
            "pass/write_only/returned_destination_transport_exit",
        ],
        "report_diag": "cannot establish assignment invariants: performed write has unknown destination origins",
    },
    {
        "id": "c1",
        "report_idx": 71,
        "witness": "pass/facts/computed_record_window_through_other_machine_exit",
        "members": [
            "pass/facts/computed_record_window_through_other_machine_exit",
            "pass/facts/computed_record_window_through_other_state_exit",
        ],
        "report_diag": "`main` calls `exercise`, which calls `window`; `window` has no executable body",
    },
    {
        "id": "c2",
        "report_idx": 111,
        "witness": "pass/facts/target_family_contract_operand/call_result",
        "members": ["pass/facts/target_family_contract_operand/call_result"],
        "report_diag": "authored Operator declaration selection",
    },
    {
        "id": "c3",
        "report_idx": 151,
        "witness": "pass/facts/match_computed_root_through_other_state_exit",
        "members": ["pass/facts/match_computed_root_through_other_state_exit"],
        "report_diag": "cannot prove requires contract for call compute",
    },
    {
        "id": "c4",
        "report_idx": 191,
        "witness": "pass/generics/generic_result_parameter_receipt",
        "members": ["pass/generics/generic_result_parameter_receipt"],
        "report_diag": "operand has no complete completed call-slot relation",
    },
]


def parse_measure() -> dict[str, tuple[str, str]]:
    data: dict[str, tuple[str, str]] = {}
    lines = MEASURE.read_text().splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        if not line.strip() or line.startswith("\t"):
            i += 1
            continue
        parts = line.split()
        if len(parts) < 2:
            i += 1
            continue
        fixture, status = parts[0], parts[1]
        diag = ""
        if i + 1 < len(lines) and lines[i + 1].startswith("\t"):
            diag = lines[i + 1][1:].strip()
        data[fixture] = (status, diag)
        i += 1
    return data


def is_fixed(fixture: str, measure: dict, ledger_line: str) -> bool:
    if fixture not in measure:
        return False
    status, _ = measure[fixture]
    tier = fixture.split("/")[0]
    m = re.search(r"regressed-from:([^\s@]+)", ledger_line)
    good = m.group(1).split("+")[0] if m else ledger_line.split()[1]
    if tier == "fail":
        return status in ("rejected", "incomplete")
    return status == good


def load_bisect(cluster_id: str) -> tuple[str, str, str, str]:
    path = ROOT / ".agents/ledger-bisect-2026-11-11" / f"bisect-c{cluster_id[1:]}-first-bad.txt"
    if not path.is_file():
        return "", "", "", "pending"
    sha = path.read_text().strip().split()[0]
    subject = subprocess.check_output(
        ["git", "-C", str(ROOT), "log", "-1", "--format=%s", sha],
        text=True,
    ).strip()
    pr = ""
    m = re.search(r"\(#(\d+)\)", subject)
    if m:
        pr = m.group(1)
    mech_path = ROOT / ".agents/ledger-bisect-2026-11-11" / f"mechanism-c{cluster_id[1:]}.txt"
    mechanism = mech_path.read_text().strip() if mech_path.is_file() else ""
    return sha, pr, mechanism, "bisected"


def ledger_line_at_pin(fixture: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(ROOT), "show", f"{PIN}:tests/omega/corpus_outcomes.txt"],
        text=True,
    ).splitlines()
    # fixed below


def main() -> None:
    corpus = subprocess.check_output(
        ["git", "-C", str(ROOT), "show", f"{PIN}:tests/omega/corpus_outcomes.txt"],
        text=True,
    )
    ledger = {
        line.split()[0]: line
        for line in corpus.splitlines()
        if line and not line.startswith("#") and "/" in line.split()[0]
    }
    measure = parse_measure()
    all_members = [m for c in CLUSTERS for m in c["members"]]
    fixed = [f for f in all_members if is_fixed(f, measure, ledger[f])]
    broken = [f for f in all_members if f not in fixed]
    pin_clusters: list[dict] = []
    for c in CLUSTERS:
        broken_m = [m for m in c["members"] if m in broken]
        if not broken_m:
            continue
        witness = c["witness"]
        if witness in broken:
            wstatus, wdiag = measure[witness]
        else:
            wstatus, wdiag = measure.get(witness, ("", ""))
        pin_clusters.append({**c, "broken_members": broken_m, "pin_diag": wdiag, "pin_status": wstatus})
    rows = []
    for pc in pin_clusters:
        sha, pr, mech, status = load_bisect(pc["id"])
        kind = "pass/run rejected" if pc["broken_members"][0].startswith("pass/") else "fail wrong reason"
        if any(measure[m][0] == "incomplete" for m in pc["broken_members"]):
            kind = "incomplete"
        rows.append(
            (len(pc["broken_members"]), kind, pc["pin_diag"][:120], pc["witness"], sha, pr, mech, status)
        )
    OUT_TSV.write_text(
        "\n".join(
            f"{fix}\t{'fixed' if fix in fixed else 'broken'}\t{measure.get(fix, ('',''))[1]}\t{next(c['id'] for c in CLUSTERS if fix in c['members'])}"
            for fix in all_members
        )
        + "\n"
    )
    body = [
        "# Ledger bisect part 31 (k=31, N=40)",
        "",
        f"Measured commit (PIN): `{PIN}`",
        "",
        "## Summary",
        "",
        f"- Legacy report clusters in slice: **5** (indices 31, 71, 111, 151, 191)",
        f"- Fixture rows re-measured: **{len(all_members)}**",
        f"- Fixed on PIN: **{len(fixed)}**",
        f"- Still broken on PIN: **{len(broken)}**",
        f"- PIN diagnostic clusters (after regroup): **{len(pin_clusters)}**",
        "",
        "## PIN clusters",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
    ]
    for r in rows:
        body.append(
            f"| {r[0]} | {r[1]} | {r[2]} | `{r[3]}` | `{r[4]}` | {r[5]} | {r[6]} | {r[7]} |"
        )
        cid = next(c["id"] for c in pin_clusters if c["witness"] == r[3])
        cl = next(c for c in pin_clusters if c["id"] == cid)
        body.append("Members:")
        for m in cl["members"]:
            st = "fixed on main" if m in fixed else "still broken"
            d = measure.get(m, ("", ""))[1][:100]
            body.append(f"- `{m}` — {st}: {d}")
        body.append("")
    body.append("## Fixed on main (PIN)")
    body.append("")
    for f in sorted(fixed):
        body.append(f"- `{f}`")
    body.append("")
    body.append("## Provenance")
    body.append("")
    body.append("- Host: linux x86_64, 4 cpus")
    body.append("- Jobs: `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`")
    body.append("- PIN ledger copy: `/tmp/ledger-bisect-pin/corpus_outcomes.txt`")
    body.append("- Measure: `mbx test --no-run -p compiler --test corpus_runner` + direct runner, filter 11 fixtures")
    OUT_MD.write_text("\n".join(body) + "\n")


if __name__ == "__main__":
    main()
