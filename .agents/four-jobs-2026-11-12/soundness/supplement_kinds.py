#!/usr/bin/env python3
"""Run only mutation kinds missing from shard TSV."""

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

SHARD = 6
TSV = HERE / f"shard-{SHARD}.tsv"


def main() -> int:
    catalog = runner.load_q7_catalog()
    seen: dict[str, set[str]] = {}
    rows: list[runner.Row] = []
    if TSV.is_file():
        with TSV.open(encoding="utf-8") as f:
            reader = csv.DictReader(f, delimiter="\t")
            for old in reader:
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
                seen.setdefault(old["fixture"], set()).add(old["mutation_kind"])
    added = 0
    for fixture in runner.list_shard_fixtures():
        rel = fixture.relative_to(REPO).as_posix()
        tier = "pass" if "/pass/" in rel else "run"
        check_root = runner.check_root_for_fixture(fixture.parent)
        cur0, _, _, _ = runner.run_omega(check_root)
        if not runner.accepts(cur0):
            continue
        original = runner.read_source(fixture)
        have = seen.get(rel, set())
        for m in runner.generate_mutations(original):
            if m.kind in have:
                continue
            added += 1
            mid = runner.mutant_id(fixture, m.kind, m.detail)
            mutant_main = runner.stage_mutant(fixture, m.text, mid)
            current_v, current_s, _, _ = runner.run_omega(mutant_main)
            route_v, route_s, route_out = runner.run_route(mutant_main)
            _, route_detail = runner.parse_route_output(route_out)
            spec = runner.read_spec_verdict(m.kind, original, m.text, m)
            (
                bucket,
                obligation,
                rfile,
                rhead,
                rquote,
                accepted,
                discard_reason,
                q7_listed,
            ) = runner.assign_bucket(
                spec, m.kind, rel, m, m.text, current_v, route_v, catalog
            )
            keep = bucket in ("confirmed-hole", "known-gap-q7-mut", "disagreement-valid")
            rel_mutant = mutant_main.relative_to(REPO).as_posix()
            if not keep:
                import shutil

                shutil.rmtree(mutant_main.parent, ignore_errors=True)
                rel_mutant = ""
            row = runner.Row(
                fixture=rel,
                tier=tier,
                kind=m.kind,
                detail=m.detail,
                bucket=bucket,
                current=current_v,
                route=route_v,
                route_detail=route_detail.replace("\t", " ")[:500],
                discard_reason=discard_reason.replace("\t", " ")[:500],
                obligation=obligation.replace("\t", " ")[:500],
                rule_file=rfile,
                rule_heading=rhead,
                rule_quote=rquote,
                accepted_by=accepted,
                hole_subtype="",
                mutant_path=rel_mutant,
                current_s=current_s,
                route_s=route_s,
                q7_audit_listed=q7_listed,
            )
            rows.append(row)
            with TSV.open("a", encoding="utf-8") as f:
                f.write(
                    f"{rel}\t{tier}\t{m.kind}\t{m.detail}\t{bucket}\t{current_v}\t{route_v}\t"
                    f"{row.route_detail}\t{row.discard_reason}\t{row.obligation}\t{rfile}\t{rhead}\t"
                    f"{rquote.replace(chr(9), ' ')[:300]}\t{accepted}\t\t{q7_listed}\t{rel_mutant}\t"
                    f"{current_s:.3f}\t{route_s:.3f}\n"
                )
            print(f"added {rel} {m.kind}")
    print("supplement rows added", added, "total", len(rows))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
