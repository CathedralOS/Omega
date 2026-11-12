#!/usr/bin/env python3
"""Replace _original_not_accepted rows with failed-run / full mutation pass at PIN."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run_shard_mutations import (  # noqa: E402
    append_tsv,
    classify,
    gather_mutations,
    run_both_checkers,
    slug,
    stage_mutant,
)

HEADER = (
    "fixture\tmutation_kind\tmutant_path\tcurrent_verdict\troute_verdict\t"
    "hand_verdict\tspec_path\tspec_quote\tobligation_line\troute_reason\treview_reason\taccepted_by\thole_subtype\n"
)


def original_skip_reason(current: str, route: str, route_reason: str) -> str:
    if current == "timeout" or route == "timeout":
        return f"original checker timeout at PIN (current={current}, route={route})"
    if current == "error" or route == "error":
        return f"original checker harness error (current={current}, route={route})"
    if current == "stopped":
        return f"original stopped at PIN (route={route or 'n/a'}; {route_reason[:200]})"
    if current == "rejected":
        return f"original rejected at PIN (route={route or 'n/a'}; {route_reason[:200]})"
    return f"original not accepted at PIN (current={current}, route={route}; {route_reason[:200]})"


def run_fixture(main: Path, tsv: Path, mut_root: Path, timeout: int) -> tuple[str, list[str]]:
    """Process one fixture; return ledger class and lines to append."""
    rel = str(main.relative_to(REPO))
    current, route, route_reason = run_both_checkers(main, timeout)
    lines: list[str] = []

    if current != "accepted":
        reason = original_skip_reason(current, route, route_reason)
        lines.append(
            "\t".join(
                [
                    rel,
                    "_no_mutations",
                    "",
                    current,
                    route,
                    "failed-run",
                    "",
                    "",
                    "",
                    route_reason.replace("\t", " ")[:500],
                    reason,
                    "",
                    "",
                ]
            )
            + "\n"
        )
        return "failed-run", lines

    source = main.read_text(encoding="utf-8")
    mutations = gather_mutations(source)
    if not mutations:
        lines.append(
            "\t".join(
                [
                    rel,
                    "_no_mutations",
                    "",
                    current,
                    route,
                    "no-mutation",
                    "",
                    "",
                    "",
                    route_reason.replace("\t", " ")[:500],
                    "generators produced no mutant kinds",
                    "",
                    "",
                ]
            )
            + "\n"
        )
        return "no-mutation", lines

    for mut in mutations:
        mdir = mut_root / slug(rel) / mut.kind
        try:
            stage_mutant(main, mdir, mut.content)
        except OSError as exc:
            lines.append(
                "\t".join(
                    [
                        rel,
                        "_mutant_generate_failed",
                        mut.kind,
                        "",
                        "",
                        "failed-run",
                        "",
                        "",
                        "",
                        "",
                        str(exc)[:200],
                        "",
                        "",
                    ]
                )
                + "\n"
            )
            continue
        mrel = str((mdir / "main.omg").relative_to(REPO))
        try:
            cur, rte, rr = run_both_checkers(mdir / "main.omg", timeout)
        except Exception as exc:  # noqa: BLE001
            lines.append(
                "\t".join(
                    [
                        rel,
                        mut.kind,
                        mrel,
                        "error",
                        "error",
                        "failed-run",
                        "",
                        "",
                        "",
                        str(exc)[:200],
                        "",
                        "",
                        "",
                    ]
                )
                + "\n"
            )
            shutil.rmtree(mdir, ignore_errors=True)
            continue

        hand, spec_path, spec_quote, obligation, review, acc, hole_subtype = classify(
            mut.kind, source, mut.content, cur, rte, rr
        )
        if hand not in {"confirmed-hole", "known-gap-q7-mut"}:
            shutil.rmtree(mdir, ignore_errors=True)
        lines.append(
            "\t".join(
                [
                    rel,
                    mut.kind,
                    mrel,
                    cur,
                    rte,
                    hand,
                    spec_path,
                    spec_quote.replace("\t", " ").replace("\n", " "),
                    obligation.replace("\t", " "),
                    rr.replace("\t", " ")[:500],
                    review.replace("\t", " ")[:500],
                    acc,
                    hole_subtype,
                ]
            )
            + "\n"
        )
    return "mutated", lines


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, required=True)
    ap.add_argument("--timeout", type=int, default=120)
    args = ap.parse_args()

    os.environ.setdefault("OMEGA_LIBRARY_ROOT", str(REPO / "source/library"))
    os.environ.setdefault("OMEGA_CORPUS_ROOT", str(REPO))

    tsv = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.tsv"
    mut_root = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}" / "mutants"
    skip: set[str] = set()
    kept: list[str] = []
    for line in tsv.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        if line.startswith("fixture\t"):
            kept.append(line + "\n")
            continue
        parts = line.split("\t")
        if len(parts) > 1 and parts[1] == "_original_not_accepted":
            skip.add(parts[0])
            continue
        kept.append(line + "\n")

    ledger: dict[str, int] = {"failed-run": 0, "no-mutation": 0, "mutated": 0}
    reasons: list[dict[str, str]] = []
    new_rows: list[str] = []
    for idx, rel in enumerate(sorted(skip), 1):
        main = REPO / rel
        if not main.exists():
            new_rows.append(
                "\t".join(
                    [
                        rel,
                        "_no_mutations",
                        "",
                        "error",
                        "error",
                        "failed-run",
                        "",
                        "",
                        "",
                        "",
                        "fixture path missing on disk",
                        "",
                        "",
                    ]
                )
                + "\n"
            )
            ledger["failed-run"] += 1
            reasons.append({"fixture": rel, "class": "failed-run", "reason": "fixture path missing"})
            continue
        klass, rows = run_fixture(main, tsv, mut_root, args.timeout)
        ledger[klass] += 1
        new_rows.extend(rows)
        if klass == "failed-run":
            parts = rows[0].split("\t")
            reasons.append(
                {
                    "fixture": rel,
                    "class": "failed-run",
                    "reason": parts[10] if len(parts) > 10 else "",
                    "current": parts[3],
                    "route": parts[4],
                }
            )
        if idx % 10 == 0:
            print(f"processed {idx}/{len(skip)} skipped originals…", flush=True)

    out_path = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.original-skip-report.json"
    out_path.write_text(json.dumps(reasons, indent=2) + "\n", encoding="utf-8")

    tsv.write_text("".join(kept) + "".join(new_rows), encoding="utf-8")
    print("reconciled skipped originals:", ledger)
    print("wrote", out_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
