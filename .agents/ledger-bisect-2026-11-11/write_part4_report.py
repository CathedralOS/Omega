#!/usr/bin/env python3
"""Write part-4.md and part-4.tsv from bisect cluster files."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(DIR / "tools"))
from bisect_test import classify_obs, observe  # noqa: E402
from prepare_bisect_expect import expected_fragments, ledger_expectation, pin_bad  # noqa: E402

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
LEDGER = Path(os.environ.get("BISECT_PIN_LEDGER", "/tmp/bisect_pin_corpus_outcomes.txt"))
OBS = DIR / "pin_observed.txt"


def expect_for(fixture: str, ledger: str, obs: str) -> dict:
    good, tier, ledger_status, ledger_diag = ledger_expectation(fixture, ledger)
    bad = pin_bad(fixture, obs)
    return {
        "witness": fixture,
        "tier": tier,
        "good_status": good,
        "ledger_status": ledger_status,
        "ledger_diag_norm": ledger_diag,
        "expected_fragments": expected_fragments(fixture),
        **bad,
    }


def main() -> None:
    prior = json.loads((DIR / "prior_slice4.json").read_text())
    fixtures: list[str] = prior["fixtures"]
    ledger = LEDGER.read_text() if LEDGER.exists() else ""
    obs = OBS.read_text()
    fixed: list[str] = []
    broken: list[str] = []
    unknown: list[str] = []
    for fixture in fixtures:
        if not ledger:
            unknown.append(fixture)
            continue
        try:
            expect = expect_for(fixture, ledger, obs)
        except KeyError:
            unknown.append(fixture)
            continue
        code = classify_obs(observe(obs, fixture), expect, "stop")
        if code == 0:
            fixed.append(fixture)
        elif code == 1:
            broken.append(fixture)
        else:
            unknown.append(fixture)

    clusters = []
    for path in sorted((DIR / "bisects").glob("cluster_*.json")):
        data = json.loads(path.read_text())
        if data.get("witness_version") != 2:
            continue
        clusters.append(data)

    owned: dict[str, str] = {}
    duplicates: list[str] = []
    for data in clusters:
        cid = str(data.get("cluster_id"))
        for fixture in data.get("owns") or []:
            if fixture in owned:
                duplicates.append(f"{fixture} in {owned[fixture]} and {cid}")
            else:
                owned[fixture] = cid

    missing = [fixture for fixture in broken if fixture not in owned]
    extra = [fixture for fixture in owned if fixture not in broken]
    reconciled = (
        not missing
        and not extra
        and not duplicates
        and not unknown
        and len(fixed) + len(broken) == len(fixtures)
    )

    lines = [
        "# Ledger bisect part 4 (k=4, N=40)",
        "",
        f"PIN: `{PIN}`",
        "",
        "Witness stop: exit 0 is the PIN-ledger outcome, exit 1 is any other outcome that ran.",
        "Witness class: exit 1 is the PIN diagnostic class, exit 0 is any other outcome that ran.",
        "Exit 125 is a build or run failure only. Ledger-only commits are not edges.",
        "Find-good is an exponential first-parent walk. Bisect is `--first-parent` between that 0 and the nearest sampled 1.",
        "",
        "## Slice reconciliation",
        "",
        f"- Slice fixtures: **{len(fixtures)}**",
        f"- Fixed (stop = 0 on the saved PIN observation): **{len(fixed)}**",
        f"- Broken (stop = 1 on the saved PIN observation): **{len(broken)}**",
        f"- Unclassified: **{len(unknown)}**",
        f"- Fixed + broken = {len(fixed) + len(broken)} (slice {len(fixtures)})",
        f"- Reconciliation: **{'OK' if reconciled else 'INCOMPLETE'}**",
        "",
    ]
    if missing:
        lines.append("Broken rows not yet owned by a finished cluster:")
        for fixture in missing:
            lines.append(f"- `{fixture}`")
        lines.append("")
    if duplicates:
        lines.append("Rows owned by more than one cluster:")
        for item in duplicates:
            lines.append(f"- {item}")
        lines.append("")
    if extra:
        lines.append("Owned rows that are not in the broken set:")
        for fixture in extra:
            lines.append(f"- `{fixture}`")
        lines.append("")

    remeasure = DIR / "remeasure_stop_witness.json"
    if remeasure.exists():
        data = json.loads(remeasure.read_text())
        lines += ["## Remeasure of #2478 and #2498 with witness stop", ""]
        for row in data.get("commits") or []:
            lines.append(
                f"- `{row['sha']}` stop={row.get('stop_rc')} class={((row.get('modes') or {}).get('class'))} {row.get('pr_subject','')}"
            )
        lines.append("")

    lines += ["## Clusters", ""]
    tsv = []
    for fixture in fixed:
        tsv.append(f"{fixture}\tfixed\t\t\t")
    for data in clusters:
        cid = data.get("cluster_id")
        lines.append(f"### Cluster {cid} — {data.get('status')}")
        lines.append("")
        lines.append(f"- Witness: `{data.get('witness')}`")
        lines.append(f"- Owns: {', '.join(f'`{item}`' for item in data.get('owns') or []) or '(none)'}")
        if data.get("mechanism"):
            lines.append(f"- Note: {data['mechanism']}")
        if data.get("fixture_added"):
            lines.append(f"- Fixture added: `{data['fixture_added']}`")
        for edge in data.get("edges") or []:
            brk = edge.get("breaking_commit") or ""
            lines.append(
                f"- Edge `{edge.get('role')}` accepted={edge.get('accepted')} "
                f"first-bad `{brk}` parent `{edge.get('parent_sha','')}` PR {edge.get('pr') or ''}"
            )
            lines.append(
                f"  - 3x first-bad `{edge.get('breaking_exit_triplet')}` 3x parent `{edge.get('parent_exit_triplet')}`"
            )
            lines.append(f"  - Good bound `{edge.get('good_commit','')}` nearest 1 `{edge.get('bad_bound','')}`")
            if edge.get("reject_reason"):
                lines.append(f"  - Not accepted: {edge['reject_reason']}")
            flippers = ", ".join(f"`{item}`" for item in edge.get("flippers") or [])
            if flippers:
                lines.append(f"  - Flippers: {flippers}")
            hunk = edge.get("diff_hunk") or ""
            if hunk:
                lines.append("")
                lines.append("```")
                lines.append(hunk)
                lines.append("```")
            if brk:
                for fixture in edge.get("flippers") or []:
                    tsv.append(
                        f"{fixture}\t{data.get('status')}\t{cid}\t{brk}\t{edge.get('pr') or ''}"
                    )
        if data.get("status") == "never-good":
            for fixture in data.get("owns") or []:
                tsv.append(f"{fixture}\tnever-good\t{cid}\t{data.get('fixture_added','')}\t")
        if data.get("status") == "unresolved":
            for fixture in data.get("owns") or []:
                tsv.append(f"{fixture}\tunresolved\t{cid}\t\t")
        non = data.get("non_flippers") or []
        if non:
            lines.append(f"- Split off: {', '.join(f'`{item}`' for item in non)}")
        lines.append("")

    owned_tsv = {line.split("\t", 1)[0] for line in tsv}
    for fixture in broken:
        if fixture not in owned_tsv:
            tsv.append(f"{fixture}\tpending\t\t\t")
    lines += [
        "## Commands",
        "",
        "- Fresh `CARGO_TARGET_DIR=/tmp/bisect-target-<sha>` per commit, one `mbx build` at a time.",
        "- `python3 .agents/ledger-bisect-2026-11-11/run_cluster.py --all`",
        "",
    ]
    (DIR / "part-4.md").write_text("\n".join(lines))
    header = "fixture\tdisposition\tcluster\tbreaking_or_fixture_add\tpr\n"
    (DIR / "part-4.tsv").write_text(header + "\n".join(tsv) + "\n")
    print(
        f"slice={len(fixtures)} fixed={len(fixed)} broken={len(broken)} "
        f"owned={len(owned)} missing={len(missing)} reconciled={reconciled}"
    )


if __name__ == "__main__":
    main()
