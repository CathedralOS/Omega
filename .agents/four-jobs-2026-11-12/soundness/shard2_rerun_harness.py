#!/usr/bin/env python3
"""Rerun mutations for fixtures misclassified as original-fail (harness) in run 1."""
from __future__ import annotations

import json
import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from shard2_mutate_run import (  # noqa: E402
    MUTATORS,
    MUT_ROOT,
    PROGRESS,
    REPO,
    append_tsv,
    check_both,
    copy_fixture_package,
    current_verdict,
    mutant_id,
    one_line_diag,
    tier,
)

OUT = Path(__file__).resolve().parent


def harness_fixtures() -> list[str]:
    prog = json.loads(PROGRESS.read_text())
    out: list[str] = []
    for rec in prog.get("stats", {}).get("run2_harness_reroute", []):
        fx = rec["fixture"]
        if current_verdict(REPO / fx) != "accepted":
            continue
        out.append(fx)
    return sorted(set(out))


def main() -> int:
    targets = harness_fixtures()
    print("rerun fixtures", len(targets), file=sys.stderr)
    MUT_ROOT.mkdir(parents=True, exist_ok=True)
    for fixture_rel in targets:
        src_main = REPO / fixture_rel
        text = src_main.read_text()
        for kind, rule_hint, fn in MUTATORS:
            mutated = fn(text)
            if mutated is None:
                continue
            mid = mutant_id(fixture_rel, kind, 0)
            mdir = MUT_ROOT / mid
            if mdir.exists():
                shutil.rmtree(mdir)
            copy_fixture_package(fixture_rel, mdir)
            (mdir / "main.omg").write_text(mutated)
            mpath = mdir / "main.omg"
            t0 = time.monotonic()
            try:
                cv, rv, route_detail, _raw = check_both(mpath)
            except Exception as exc:
                append_tsv(
                    {
                        "fixture": fixture_rel,
                        "tier": tier(fixture_rel),
                        "mutation_kind": kind,
                        "mutant_path": "",
                        "current_verdict": "error",
                        "route_verdict": "error",
                        "run_disposition": "run_failed",
                        "route_rejection_detail": one_line_diag(str(exc)),
                        "rule_hint": rule_hint,
                        "seconds": f"{time.monotonic() - t0:.2f}",
                    }
                )
                shutil.rmtree(mdir, ignore_errors=True)
                continue
            secs = time.monotonic() - t0
            if cv == "timeout" or rv == "timeout":
                run_disposition = "timeout"
                shutil.rmtree(mdir, ignore_errors=True)
            elif cv in ("rejected", "stopped") and rv in (
                "rejected",
                "incomplete",
                "defect",
                "outside",
                "fallback",
                "no-entry",
                "unreached",
            ):
                run_disposition = "rejected_both"
                shutil.rmtree(mdir, ignore_errors=True)
            elif cv == "accepted" or rv == "accepted":
                run_disposition = "checker_accepts"
            else:
                run_disposition = "neither_accepts"
                shutil.rmtree(mdir, ignore_errors=True)
            append_tsv(
                {
                    "fixture": fixture_rel,
                    "tier": tier(fixture_rel),
                    "mutation_kind": kind,
                    "mutant_path": (
                        str(mpath.relative_to(REPO))
                        if run_disposition == "checker_accepts"
                        else ""
                    ),
                    "current_verdict": cv,
                    "route_verdict": rv,
                    "run_disposition": run_disposition,
                    "route_rejection_detail": route_detail,
                    "rule_hint": rule_hint,
                    "seconds": f"{secs:.2f}",
                }
            )
    print("done")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
