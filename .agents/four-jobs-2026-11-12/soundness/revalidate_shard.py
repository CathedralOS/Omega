#!/usr/bin/env python3
"""Re-run fix3 classification on an existing shard TSV."""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from soundness_validate import classify  # noqa: E402
from entry_probe import probe_subtype  # noqa: E402

HEADER = (
    "fixture\tmutation_kind\tmutant_path\tcurrent_verdict\troute_verdict\t"
    "hand_verdict\tspec_path\tspec_quote\tobligation_line\troute_reason\treview_reason\taccepted_by\thole_subtype\n"
)

REPO_REF = REPO


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, required=True)
    args = ap.parse_args()

    out = REPO / ".agents/four-jobs-2026-11-12/soundness"
    tsv_path = out / f"shard-{args.shard}.tsv"
    if not tsv_path.exists():
        print("missing tsv", tsv_path)
        return 1

    rows_out: list[str] = [HEADER]
    buckets: Counter[str] = Counter()

    for line in tsv_path.read_text().splitlines()[1:]:
        parts = line.split("\t")
        while len(parts) < 13:
            parts.append("")
        fixture, kind = parts[0], parts[1]
        orig_path = REPO / fixture
        mutant_path = REPO / parts[2] if parts[2] else None

        if kind in {"_original_not_accepted"}:
            rows_out.append("\t".join(parts[:13]) + "\n")
            continue
        if kind == "_no_mutations":
            parts[5] = "no-mutation"
            rows_out.append("\t".join(parts[:13]) + "\n")
            buckets["no-mutation"] += 1
            continue
        if kind.startswith("_"):
            parts[5] = "failed-run"
            rows_out.append("\t".join(parts[:13]) + "\n")
            buckets["failed-run"] += 1
            buckets["mutant-total"] += 1
            continue

        original = orig_path.read_text(encoding="utf-8") if orig_path.exists() else ""
        mutant = ""
        if mutant_path and mutant_path.exists():
            mutant = mutant_path.read_text(encoding="utf-8")
        elif original and kind == "remove_mut":
            mutant = re.sub(r"\blet\s+mut\s+", "let ", original, count=1)

        hand, spec_path, spec_quote, obligation, review, acc, hole_subtype = classify(
            kind, original, mutant, parts[3], parts[4], parts[9]
        )
        if (
            kind == "drop_requires"
            and hand == "confirmed-hole"
            and mutant_path
            and mutant_path.exists()
        ):
            import os

            from run_shard_mutations import run_both_checkers  # noqa: E402

            def accepted(p: Path) -> bool:
                os.environ.setdefault("OMEGA_LIBRARY_ROOT", str(REPO_REF / "source/library"))
                os.environ.setdefault("OMEGA_CORPUS_ROOT", str(REPO_REF))
                cur, _, _ = run_both_checkers(p, 60)
                return cur == "accepted"

            built = probe_subtype(mutant_path, original, accepted)
            if built in {"inferred-entry", "unchecked", "probe-not-built"}:
                hole_subtype = built
        parts[5:13] = [
            hand,
            spec_path,
            spec_quote.replace("\t", " ").replace("\n", " "),
            obligation.replace("\t", " "),
            parts[9],
            review.replace("\t", " ")[:500],
            acc,
            hole_subtype,
        ]
        rows_out.append("\t".join(parts[:13]) + "\n")
        buckets[hand] += 1
        buckets["mutant-total"] += 1

    tsv_path.write_text("".join(rows_out), encoding="utf-8")
    meta = out / f"shard-{args.shard}.meta.json"
    payload = {}
    if meta.exists():
        payload = json.loads(meta.read_text())
    payload["revalidated_mutants"] = buckets["mutant-total"]
    meta.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    print(f"mutant_total={buckets['mutant-total']}")
    for bucket in (
        "confirmed-hole",
        "known-gap-q7-mut",
        "disagreement-valid",
        "valid-route-outside",
        "discarded-valid",
        "rejected-both",
        "timeout",
        "failed-run",
        "no-mutation",
    ):
        print(f"{bucket}={buckets.get(bucket, 0)}")
    assert buckets["mutant-total"] == sum(
        buckets.get(b, 0)
        for b in (
            "confirmed-hole",
            "known-gap-q7-mut",
            "disagreement-valid",
            "valid-route-outside",
            "discarded-valid",
            "rejected-both",
            "timeout",
            "failed-run",
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
