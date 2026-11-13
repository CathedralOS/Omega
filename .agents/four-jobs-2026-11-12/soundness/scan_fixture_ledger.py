#!/usr/bin/env python3
"""Build shard fixture ledger (original check outcomes)."""

from __future__ import annotations

import csv
import importlib.util
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
spec = importlib.util.spec_from_file_location("runner", HERE / "run_shard_mutations.py")
runner = importlib.util.module_from_spec(spec)
assert spec and spec.loader
sys.modules["runner"] = runner
spec.loader.exec_module(runner)

SHARD = 6
OUT = HERE / f"shard-{SHARD}.fixtures.tsv"
MD = HERE / f"shard-{SHARD}.md"


def main() -> int:
    fixtures = runner.list_shard_fixtures()
    ledger: list[runner.FixtureLedgerRow] = []
    existing_kinds: dict[str, set[tuple[str, str]]] = {}
    tsv = HERE / f"shard-{SHARD}.tsv"
    if tsv.is_file():
        with tsv.open(encoding="utf-8") as f:
            for row in csv.DictReader(f, delimiter="\t"):
                existing_kinds.setdefault(row["fixture"], set()).add(
                    (row["mutation_kind"], row["detail"])
                )
    mutated_fixtures = set(existing_kinds.keys())
    for fixture in fixtures:
        rel = fixture.relative_to(REPO).as_posix()
        tier = "pass" if "/pass/" in rel else "run"
        if rel in mutated_fixtures:
            ledger.append(runner.FixtureLedgerRow(rel, tier, "mutated", ""))
            continue
        check_root = runner.check_root_for_fixture(fixture.parent)
        verdict, _, _, _ = runner.run_omega(check_root)
        if verdict == "timeout" or verdict == "error":
            ledger.append(
                runner.FixtureLedgerRow(rel, tier, "failed-run", runner.diagnose_check(check_root))
            )
            continue
        if not runner.accepts(verdict):
            ledger.append(
                runner.FixtureLedgerRow(
                    rel, tier, "original-not-accepted", runner.diagnose_check(check_root)
                )
            )
            continue
        original = runner.read_source(fixture)
        mutations = runner.generate_mutations(original)
        if not mutations and rel not in existing_kinds:
            ledger.append(runner.FixtureLedgerRow(rel, tier, "no-mutation", ""))
        elif rel in existing_kinds or mutations:
            ledger.append(runner.FixtureLedgerRow(rel, tier, "mutated", ""))
    runner.write_fixture_ledger(OUT, ledger)
    print(
        "ledger",
        Counter(r.outcome for r in ledger),
        "sum",
        len(ledger),
    )
    if MD.is_file() and tsv.is_file():
        import csv as csvm

        rows: list[runner.Row] = []
        with tsv.open(encoding="utf-8") as f:
            for old in csvm.DictReader(f, delimiter="\t"):
                rows.append(
                    runner.Row(
                        fixture=old["fixture"],
                        tier=old["tier"],
                        kind=old["mutation_kind"],
                        detail=old["detail"],
                        bucket=old["bucket"],
                        current=old["current_verdict"],
                        route=old["route_verdict"],
                        route_detail=old.get("route_detail", ""),
                        discard_reason=old.get("discard_reason", ""),
                        obligation=old.get("obligation", ""),
                        rule_file=old.get("rule_file", ""),
                        rule_heading=old.get("rule_heading", ""),
                        rule_quote=old.get("rule_quote", ""),
                        accepted_by=old["accepted_by"],
                        hole_subtype=old.get("hole_subtype", ""),
                        mutant_path=old.get("mutant_path", ""),
                        current_s=float(old.get("current_seconds", 0) or 0),
                        route_s=float(old.get("route_seconds", 0) or 0),
                        q7_audit_listed=old.get("q7_audit_listed", ""),
                    )
                )
        import os

        os.environ["SOUNDNESS_SKIP_HAND"] = "1"
        runner.write_md(
            MD,
            len(fixtures),
            ledger,
            sum(1 for r in ledger if r.outcome == "no-mutation"),
            len(rows),
            rows,
            runner.load_q7_catalog(),
            finished=True,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
