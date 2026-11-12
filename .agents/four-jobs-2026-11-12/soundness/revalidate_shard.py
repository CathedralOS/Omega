#!/usr/bin/env python3
"""Re-run lead-review classification on an existing shard TSV."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from soundness_validate import classify  # noqa: E402

HEADER = (
    "fixture\tmutation_kind\tmutant_path\tcurrent_verdict\troute_verdict\t"
    "hand_verdict\tspec_path\tspec_quote\tobligation_line\troute_reason\treview_reason\n"
)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, required=True)
    args = ap.parse_args()

    out = REPO / ".agents/four-jobs-2026-11-12/soundness"
    tsv_path = out / f"shard-{args.shard}.tsv"
    if not tsv_path.exists():
        print("missing tsv", tsv_path)
        return 1

    old_confirmed = 0
    downgraded = 0
    rows_out: list[str] = [HEADER]
    buckets: Counter[str] = Counter()
    kinds_confirmed: Counter[str] = Counter()
    kinds_q7: Counter[str] = Counter()

    for line in tsv_path.read_text().splitlines()[1:]:
        parts = line.split("\t")
        while len(parts) < 11:
            parts.append("")
        fixture, kind = parts[0], parts[1]
        orig_path = REPO / fixture
        mutant_path = REPO / parts[2] if parts[2] else None

        if kind in {"_original_not_accepted", "_no_mutations"} or kind.startswith("_"):
            if kind == "_mutant_generate_failed":
                parts[5] = "mutant-generate-failed"
                buckets["mutant-generate-failed"] += 1
                buckets["mutant-total"] += 1
            else:
                parts[5] = parts[5] or "discarded-valid"
            rows_out.append("\t".join(parts[:11]) + "\n")
            continue

        if parts[5] == "confirmed-invalid":
            old_confirmed += 1

        original = orig_path.read_text(encoding="utf-8") if orig_path.exists() else ""
        mutant = ""
        if mutant_path and mutant_path.exists():
            mutant = mutant_path.read_text(encoding="utf-8")
        elif original and kind == "remove_mut":
            import re

            mutant = re.sub(r"\blet\s+mut\s+", "let ", original, count=1)

        route_reason = parts[9] or f"route={parts[4]}"
        prior = parts[5]
        verdict, spec_path, spec_quote, obligation, review = classify(
            kind, original, mutant, parts[3], parts[4], route_reason
        )
        if prior == "confirmed-invalid" and verdict != "confirmed-invalid":
            downgraded += 1

        parts[5] = verdict
        parts[6] = spec_path
        parts[7] = spec_quote.replace("\t", " ").replace("\n", " ")
        parts[8] = obligation.replace("\t", " ")
        parts[10] = review.replace("\t", " ")[:500]
        rows_out.append("\t".join(parts[:11]) + "\n")

        if parts[5] == "mutant-run-failed":
            buckets["mutant-run-failed"] += 1
        buckets[verdict] += 1
        buckets["mutant-total"] += 1
        if verdict == "confirmed-invalid":
            kinds_confirmed[kind] += 1
        if verdict == "known-gap-q7-mut":
            kinds_q7[kind] += 1

    tsv_path.write_text("".join(rows_out), encoding="utf-8")
    meta = out / f"shard-{args.shard}.meta.json"
    payload = {}
    if meta.exists():
        payload = json.loads(meta.read_text())
    payload.update(
        {
            "downgraded_from_confirmed": downgraded,
            "old_confirmed_total": old_confirmed,
            "revalidated_mutants": buckets["mutant-total"],
        }
    )
    meta.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    print(f"downgraded_from_confirmed={downgraded} (was {old_confirmed})")
    print(f"mutant_total={buckets['mutant-total']}")
    for bucket in (
        "confirmed-invalid",
        "known-gap-q7-mut",
        "discarded-valid",
        "discarded-no-rule",
        "mutant-run-failed",
        "mutant-generate-failed",
    ):
        print(f"{bucket}={buckets.get(bucket, 0)}")
    print("confirmed_by_kind", dict(kinds_confirmed))
    print("q7_by_kind", dict(kinds_q7))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
