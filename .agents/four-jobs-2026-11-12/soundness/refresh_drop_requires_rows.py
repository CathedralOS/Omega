#!/usr/bin/env python3
"""Re-stage, checker-run, and reclassify every drop_requires TSV row at PIN."""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from entry_probe import probe_subtype  # noqa: E402
from requires_span import drop_first_requires  # noqa: E402
from run_shard_mutations import classify, run_both_checkers, slug, stage_mutant  # noqa: E402

HEADER = (
    "fixture\tmutation_kind\tmutant_path\tcurrent_verdict\troute_verdict\t"
    "hand_verdict\tspec_path\tspec_quote\tobligation_line\troute_reason\treview_reason\taccepted_by\thole_subtype\n"
)


def probe_row(mutant_path: Path, original: str, timeout: int) -> tuple[str, str]:
    import os

    def accepted(p: Path) -> bool:
        os.environ.setdefault("OMEGA_LIBRARY_ROOT", str(REPO / "source/library"))
        os.environ.setdefault("OMEGA_CORPUS_ROOT", str(REPO))
        cur, _, _ = run_both_checkers(p, min(timeout, 90))
        return cur == "accepted"

    return probe_subtype(mutant_path, original, accepted), ""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, required=True)
    ap.add_argument("--timeout", type=int, default=120)
    args = ap.parse_args()

    tsv = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.tsv"
    mut_root = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}" / "mutants"
    lines = tsv.read_text(encoding="utf-8").splitlines()
    out: list[str] = [lines[0] + "\n" if lines else HEADER]
    done = 0
    for line in lines[1:]:
        parts = line.split("\t")
        while len(parts) < 13:
            parts.append("")
        if parts[1] != "drop_requires":
            out.append("\t".join(parts[:13]) + "\n")
            continue
        rel = parts[0]
        main = REPO / rel
        if not main.exists():
            out.append("\t".join(parts[:13]) + "\n")
            continue
        source = main.read_text(encoding="utf-8")
        mutated = drop_first_requires(source)
        if not mutated or mutated == source:
            parts[5] = "failed-run"
            parts[10] = "drop_requires generator produced no mutant"
            out.append("\t".join(parts[:13]) + "\n")
            continue
        mdir = mut_root / slug(rel) / "drop_requires"
        stage_mutant(main, mdir, mutated)
        mrel = str((mdir / "main.omg").relative_to(REPO))
        current, route, route_reason = run_both_checkers(mdir / "main.omg", args.timeout)
        hand, spec_path, spec_quote, obligation, review, acc, hole_subtype = classify(
            "drop_requires", source, mutated, current, route, route_reason
        )
        if hand == "confirmed-hole":
            built, _ = probe_row(mdir / "main.omg", source, args.timeout)
            base = hole_subtype or "body-requires"
            hole_subtype = f"{base}:{built}"
            if built == "probe-not-built":
                hand = "discarded-valid"
                review = (
                    "drop_requires accepted but violating-entry probe not built; "
                    "cannot claim modular-entry hole"
                )
            elif built not in {"inferred-entry", "unchecked"}:
                hand = "discarded-valid"
                review = f"unexpected probe subtype `{built}`"
        if hand not in {"confirmed-hole", "known-gap-q7-mut"}:
            shutil.rmtree(mdir, ignore_errors=True)
            mrel = parts[2]
        parts[2:13] = [
            mrel,
            current,
            route,
            hand,
            spec_path,
            spec_quote.replace("\t", " ").replace("\n", " "),
            obligation.replace("\t", " "),
            route_reason.replace("\t", " ")[:500],
            review.replace("\t", " ")[:500],
            acc,
            hole_subtype,
        ]
        out.append("\t".join(parts[:13]) + "\n")
        done += 1
        if done % 50 == 0:
            print(f"refreshed {done} drop_requires rows…", flush=True)
    tsv.write_text("".join(out), encoding="utf-8")
    print(f"refreshed {done} drop_requires rows total")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
