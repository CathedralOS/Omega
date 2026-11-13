#!/usr/bin/env python3
"""Re-bucket shard-6 TSV (canonical kinds, Q7, buckets) without re-running checks."""

from __future__ import annotations

import csv
import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
spec = importlib.util.spec_from_file_location("runner", HERE / "run_shard_mutations.py")
runner = importlib.util.module_from_spec(spec)
assert spec and spec.loader
sys.modules["runner"] = runner
spec.loader.exec_module(runner)

from q7_gaps import load_q7_catalog  # noqa: E402

SHARD = 6
TSV = HERE / f"shard-{SHARD}.tsv"
OUT = HERE / f"shard-{SHARD}.rebucketed.tsv"


def normalize_kind(kind: str) -> str:
    return runner.KIND_ALIASES.get(kind, kind)


def normalize_detail(kind: str, detail: str) -> str:
    if kind == "drop_requires" and detail == "delete first requires line":
        return "delete first body requires line"
    if kind == "remove_guard_transition" and "when" in detail:
        return detail.replace("remove_guard", "remove_guard_transition")
    return detail


def row_from_old(old: dict[str, str], catalog) -> runner.Row:
    kind = normalize_kind(old.get("mutation_kind", old.get("kind", "")))
    detail = normalize_detail(kind, old["detail"])
    fixture = old["fixture"]
    fixture_path = REPO / fixture
    regen = runner.regen_mutant_text(fixture, kind, detail)
    if not regen and kind in runner.KIND_ALIASES.values():
        for alias, canon in runner.KIND_ALIASES.items():
            if canon == kind:
                regen = runner.regen_mutant_text(
                    fixture,
                    alias,
                    detail.replace("body requires", "requires").replace(
                        "remove_guard_transition", "remove_guard"
                    ),
                )
                if regen:
                    break
    if not regen:
        original = runner.read_source(fixture_path)
        mutant = original
        mutation = runner.Mutation(kind, detail, original, {})
    else:
        original, mutant = regen
        mutation = next(
            (
                m
                for m in runner.generate_mutations(original)
                if m.kind == kind and m.detail == detail
            ),
            runner.Mutation(kind, detail, mutant, {}),
        )
    spec = runner.read_spec_verdict(kind, original, mutant, mutation)
    current = old["current_verdict"]
    route = old["route_verdict"]
    bucket, obligation, rfile, rhead, rquote, accepted, discard, q7_listed = runner.assign_bucket(
        spec, kind, fixture, mutation, mutant, current, route, catalog
    )
    if bucket == "confirmed-hole" and runner.spec_allows_runtime_trap(kind, mutant):
        bucket = "discarded-valid"
        discard = (
            "invalid under read but explicit crash arm may trap at runtime; "
            "not a compile-time hole"
        )
        rfile = rhead = rquote = ""
    subtype = old.get("hole_subtype", "") or ("n/a" if bucket == "confirmed-hole" else "")
    return runner.Row(
        fixture=fixture,
        tier=old["tier"],
        kind=kind,
        detail=detail,
        bucket=bucket,
        current=current,
        route=route,
        route_detail=old.get("route_detail", ""),
        discard_reason=discard or old.get("discard_reason", ""),
        obligation=obligation or old.get("obligation", ""),
        rule_file=rfile,
        rule_heading=rhead,
        rule_quote=rquote,
        accepted_by=accepted,
        hole_subtype=subtype,
        mutant_path=old.get("mutant_path", ""),
        current_s=float(old.get("current_seconds", 0) or 0),
        route_s=float(old.get("route_seconds", 0) or 0),
        q7_audit_listed=q7_listed,
    )


def main() -> int:
    import os

    os.environ.setdefault("SOUNDNESS_SKIP_HAND", "1")
    catalog = load_q7_catalog()
    rows: list[runner.Row] = []
    with TSV.open(encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        for old in reader:
            rows.append(row_from_old(old, catalog))
    header = (
        "fixture\ttier\tmutation_kind\tdetail\tbucket\tcurrent_verdict\troute_verdict\t"
        "route_detail\tdiscard_reason\tobligation\trule_file\trule_heading\trule_quote\t"
        "accepted_by\thole_subtype\tq7_audit_listed\tmutant_path\tcurrent_seconds\troute_seconds\n"
    )
    with OUT.open("w", encoding="utf-8") as f:
        f.write(header)
        for r in rows:
            f.write(
                f"{r.fixture}\t{r.tier}\t{r.kind}\t{r.detail}\t{r.bucket}\t{r.current}\t{r.route}\t"
                f"{r.route_detail}\t{r.discard_reason}\t{r.obligation}\t{r.rule_file}\t{r.rule_heading}\t"
                f"{r.rule_quote.replace(chr(9), ' ')[:300]}\t{r.accepted_by}\t{r.hole_subtype}\t"
                f"{r.q7_audit_listed}\t{r.mutant_path}\t{r.current_s:.3f}\t{r.route_s:.3f}\n"
            )
    OUT.replace(TSV)
    md = HERE / f"shard-{SHARD}.md"
    ledger_path = HERE / f"shard-{SHARD}.fixtures.tsv"
    ledger: list[runner.FixtureLedgerRow] = []
    if ledger_path.is_file():
        with ledger_path.open(encoding="utf-8") as f:
            for line in csv.DictReader(f, delimiter="\t"):
                ledger.append(
                    runner.FixtureLedgerRow(
                        line["fixture"], line["tier"], line["outcome"], line.get("diagnostic", "")
                    )
                )
    runner.write_md(
        md,
        len(runner.list_shard_fixtures()),
        ledger,
        0,
        len(rows),
        rows,
        catalog,
        finished=True,
    )
    print(f"rebucketed {len(rows)} rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
