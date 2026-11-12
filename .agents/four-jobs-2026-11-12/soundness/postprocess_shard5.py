#!/usr/bin/env python3
"""Dedupe shard TSV, tighten remove_mut hand-checks, regenerate shard-5.md."""
from __future__ import annotations

import re
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 5
OUT = Path(__file__).resolve().parent
TSV = OUT / f"shard-{SHARD}.tsv"
MD = OUT / f"shard-{SHARD}.md"


def remove_mut_invalid(fixture_rel: str, kind: str) -> bool:
    if kind != "remove_mut":
        return True
    src = (REPO / fixture_rel).read_text(encoding="utf-8")
    m = re.search(r"\blet mut (\w+)", src)
    if not m:
        return False
    name = re.escape(m.group(1))
    return bool(
        re.search(
            rf"(?:&mut\s+{name}\b|&write\s+{name}\b|{name}\s*=(?!=)|{name}\[[^\]]+\]\s*=)",
            src,
        )
    )


def load_rows() -> list[list[str]]:
    lines = TSV.read_text(encoding="utf-8").splitlines()
    header = lines[0].split("\t")
    rows: dict[tuple[str, str], list[str]] = {}
    for line in lines[1:]:
        cols = line.split("\t")
        if len(cols) < len(header):
            continue
        key = (cols[0], cols[2])
        rows[key] = cols  # keep last
    return list(rows.values())


def main() -> None:
    rows = load_rows()
    counts = {
        "mutants_checked": len(rows),
        "confirmed": 0,
        "discarded_valid_recheck": 0,
        "discarded_no_rule": 0,
        "both_reject": 0,
    }
    out_rows: list[list[str]] = []
    for cols in rows:
        fixture, kind, hand = cols[0], cols[2], cols[7]
        if hand == "confirmed-invalid" and not remove_mut_invalid(fixture, kind):
            cols[7] = "discarded-valid"
            counts["discarded_valid_recheck"] += 1
            counts["both_reject"] += 1
        elif hand == "confirmed-invalid":
            counts["confirmed"] += 1
            out_rows.append(cols)
        elif hand == "discarded-both-reject":
            counts["both_reject"] += 1
        elif hand == "discarded-no-rule":
            counts["discarded_no_rule"] += 1

    header = (
        "fixture\ttier\tmutation_kind\tmutant_path\tcurrent_verdict\t"
        "route_verdict\tagree\thand_verdict\tspec_file\tspec_quote\t"
        "current_s\troute_s\n"
    )
    with TSV.open("w", encoding="utf-8") as f:
        f.write(header)
        for cols in sorted(rows, key=lambda c: c[0]):
            f.write("\t".join(cols) + "\n")

    md_lines = [
        f"# Soundness mutations shard {SHARD} (PIN {PIN})",
        "",
        f"Measured commit: `{PIN}`",
        "Host: cursor",
        f"Shard selection: fixture index `i % 8 == {SHARD}` over pass/ and run/ (1130 fixtures).",
        "",
        "## Counts",
        "- fixtures in shard: 1130",
        f"- mutants checked (deduped by fixture+kind): {counts['mutants_checked']}",
        f"- both checkers reject (discarded): {counts['both_reject']}",
        f"- confirmed-invalid holes/disagreements: {counts['confirmed']}",
        f"- discarded-valid after remove_mut re-check: {counts['discarded_valid_recheck']}",
        f"- discarded-no-rule: {counts['discarded_no_rule']}",
        "",
        "## Commands",
        "```bash",
        f"cd {REPO}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison",
        "OMEGA_LIBRARY_ROOT=$PWD/source/library timeout 120 ./target/debug/omega --check <mutant>/main.omg",
        "timeout 120 ./target/debug/examples/route_one_comparison <mutant>/main.omg",
        "python3 .agents/four-jobs-2026-11-12/soundness/run_shard5.py",
        "python3 .agents/four-jobs-2026-11-12/soundness/postprocess_shard5.py",
        "```",
        "",
        "## Confirmed holes and disagreements",
        "",
    ]
    for cols in sorted(out_rows, key=lambda c: c[0]):
        fixture, kind, mpath, cur, rt = cols[0], cols[2], cols[3], cols[4], cols[5]
        spec_file, spec_quote = cols[8], cols[9]
        md_lines += [
            f"### `{kind}` — `{fixture}`",
            f"- Original: `{fixture}`",
            f"- Mutant: `{mpath}`",
            f"- Current checker: **{cur}**; route: **{rt}**",
            f"- Spec rule ({spec_file}):",
            f"> {spec_quote}",
            "",
        ]
    MD.write_text("\n".join(md_lines), encoding="utf-8")
    print(counts)


if __name__ == "__main__":
    main()
