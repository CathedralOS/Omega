#!/usr/bin/env python3
"""Re-run checkers and classification on existing shard TSV mutant rows."""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from entry_probe import probe_subtype  # noqa: E402
from run_shard_mutations import run_both_checkers  # noqa: E402
from soundness_validate import classify  # noqa: E402

HEADER = (
    "fixture\tmutation_kind\tmutant_path\tcurrent_verdict\troute_verdict\t"
    "hand_verdict\tspec_path\tspec_quote\tobligation_line\troute_reason\treview_reason\taccepted_by\thole_subtype\n"
)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, required=True)
    ap.add_argument("--timeout", type=int, default=120)
    ap.add_argument("--kinds", default="", help="comma-separated kinds to rerun checkers on (default: all mutants)")
    ap.add_argument("--verify-pin", action="store_true", help="assert route_one_comparison exists at PIN build")
    args = ap.parse_args()

    route = REPO / "target/debug/examples/route_one_comparison"
    if args.verify_pin and not route.exists():
        print("missing PIN checker binary", route, file=sys.stderr)
        return 1

    tsv = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.tsv"
    kind_filter = {k.strip() for k in args.kinds.split(",") if k.strip()}

    rows_out = [HEADER]
    rerun = 0
    for line in tsv.read_text(encoding="utf-8").splitlines()[1:]:
        parts = line.split("\t")
        while len(parts) < 13:
            parts.append("")
        kind = parts[1]
        if kind.startswith("_") or kind == "_original_not_accepted":
            rows_out.append("\t".join(parts[:13]) + "\n")
            continue
        if kind_filter and kind not in kind_filter:
            rows_out.append("\t".join(parts[:13]) + "\n")
            continue

        orig_path = REPO / parts[0]
        mutant_path = REPO / parts[2] if parts[2] else None
        original = orig_path.read_text(encoding="utf-8") if orig_path.exists() else ""
        mutant = ""
        if mutant_path and mutant_path.exists():
            mutant = mutant_path.read_text(encoding="utf-8")
        elif kind == "remove_mut" and original:
            mutant = re.sub(r"\blet\s+mut\s+", "let ", original, count=1)

        if mutant_path and mutant_path.exists():
            current, route_v, route_reason = run_both_checkers(mutant_path, args.timeout)
            rerun += 1
        else:
            current, route_v, route_reason = parts[3], parts[4], parts[9]

        hand, spec_path, spec_quote, obligation, review, acc, hole_subtype = classify(
            kind, original, mutant, current, route_v, route_reason
        )
        if (
            kind == "drop_requires"
            and hand == "confirmed-hole"
            and mutant_path
            and mutant_path.exists()
        ):
            def accepted(p: Path) -> bool:
                os.environ.setdefault("OMEGA_LIBRARY_ROOT", str(REPO / "source/library"))
                os.environ.setdefault("OMEGA_CORPUS_ROOT", str(REPO))
                cur, _, _ = run_both_checkers(p, min(args.timeout, 90))
                return cur == "accepted"

            probe = probe_subtype(mutant_path, original, accepted)
            if probe in {"inferred-entry", "unchecked", "probe-not-built"}:
                base = hole_subtype.split(":")[-1] if ":" in hole_subtype else hole_subtype
                if base.endswith("-requires"):
                    hole_subtype = f"{base}:{probe}"
                else:
                    hole_subtype = f"{hole_subtype}:{probe}" if hole_subtype else probe

        parts[3:13] = [
            current,
            route_v,
            hand,
            spec_path,
            spec_quote.replace("\t", " ").replace("\n", " "),
            obligation.replace("\t", " "),
            route_reason.replace("\t", " ")[:500],
            review.replace("\t", " ")[:500],
            acc,
            hole_subtype,
        ]
        rows_out.append("\t".join(parts[:13]) + "\n")
        if rerun and rerun % 100 == 0:
            print(f"rerun checkers on {rerun} rows…", flush=True)

    tsv.write_text("".join(rows_out), encoding="utf-8")
    print(f"reran checkers on {rerun} mutant rows; tsv updated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
