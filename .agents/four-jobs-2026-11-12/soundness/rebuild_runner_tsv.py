#!/usr/bin/env python3
"""Incremental rebuild of runner TSV: backfill missing fixtures, preserve good rows."""

from __future__ import annotations

import hashlib
import sys
from collections import Counter
from pathlib import Path

_SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPT_DIR))

from run_shard_mutations import (  # noqa: E402
    PIN,
    SHARD,
    TSV_PATH,
    PROGRESS_PATH,
    mutations_for,
    materialize_mutant,
    run_omega_check,
    run_route,
    shard_fixtures,
    Mutation,
)

REPO = Path(__file__).resolve().parents[3]


def _sanitize_field(s: str) -> str:
    return s.replace("\t", " ").replace("\n", " ").replace("\r", " ")[:500]


def load_good_rows() -> dict[str, list[list[str]]]:
    by: dict[str, list[list[str]]] = {}
    if not TSV_PATH.exists():
        return by
    for line in TSV_PATH.read_text().splitlines():
        if not line or line.startswith("#") or line.startswith("original_fixture"):
            continue
        p = line.split("\t")
        if not p[0].startswith("tests/"):
            continue
        if len(p) < 9:
            continue
        row = [_sanitize_field(x) for x in p[:9]]
        by.setdefault(row[0], []).append(row)
    return by


def row_for_mut(main: Path, mut: Mutation) -> list[str]:
    rel = main.relative_to(REPO).as_posix()
    mpath = materialize_mutant(main, mut)
    cur_v, route_v, route_reason, route_s = run_route(mpath)
    omg_v = "skipped"
    omg_s = 0.0
    needs_omega = (
        cur_v == "accepted"
        or route_v == "accepted"
        or (
            cur_v in ("accepted", "rejected")
            and route_v in ("accepted", "rejected", "incomplete", "fallback")
            and cur_v != route_v
        )
    )
    if needs_omega:
        omg_v, omg_s = run_omega_check(mpath)
    accept = omg_v == "accepted" or cur_v == "accepted" or route_v == "accepted"
    disagree = (
        omg_v in ("accepted", "rejected")
        and cur_v in ("accepted", "rejected")
        and omg_v != cur_v
    ) or (
        cur_v in ("accepted", "rejected")
        and route_v in ("accepted", "rejected", "incomplete", "fallback")
        and cur_v != route_v
    )
    return [
        rel,
        mut.kind,
        mpath.relative_to(REPO).as_posix(),
        omg_v,
        cur_v,
        route_v,
        _sanitize_field(route_reason),
        f"{max(omg_s, route_s):.2f}",
        "yes" if accept or disagree else "no",
    ]


def mutations_pre_dedupe_count(text: str) -> int:
    from run_shard_mutations import mutations_for as mf
    import run_shard_mutations as rsm

    orig = rsm.mutations_for
    pre: list[Mutation] = []

    def capture(_text: str) -> list[Mutation]:
        m = mf(_text)
        pre.extend(m)
        return m

    # Approximate: unique list length vs recount adds in accounting script
    return len(mf(text))  # deduped already; use separate accounting in review


def fixture_rows(main: Path, existing: list[list[str]]) -> tuple[list[list[str]], Counter]:
    acct = Counter()
    text = main.read_text()
    muts = mutations_for(text)
    rel = main.relative_to(REPO).as_posix()
    if not muts:
        acct["no_mutation"] = 1
        return [
            [
                rel,
                "no-mutation",
                "",
                "skipped",
                "skipped",
                "skipped",
                "no-applicable-mutations",
                "0.00",
                "no",
            ]
        ], acct

    by_kind_path = {(r[1], r[2]) for r in existing}
    rows: list[list[str]] = []
    for mut in muts:
        digest = hashlib.sha256(mut.source.encode()).hexdigest()[:10]
        rel_m = materialize_mutant(main, mut)
        mrel = rel_m.relative_to(REPO).as_posix()
        key = (mut.kind, mrel)
        hit = [r for r in existing if r[1] == mut.kind and r[2] == mrel]
        if hit:
            rows.append(hit[0])
            acct["reused_row"] += 1
            continue
        rows.append(row_for_mut(main, mut))
        acct["new_checker_run"] += 1
    return rows, acct


def main() -> int:
    fixtures = shard_fixtures()
    good = load_good_rows()
    out_rows: list[list[str]] = []
    acct: Counter = Counter()

    for i, main in enumerate(fixtures):
        rel = main.relative_to(REPO).as_posix()
        existing = good.get(rel, [])
        rows, fa = fixture_rows(main, existing)
        if not existing:
            acct["fixture_backfilled"] += 1
        if fa.get("no_mutation"):
            acct["fixtures_no_mutation"] += 1
        else:
            acct["fixtures_with_mutants"] += 1
        out_rows.extend(rows)
        acct.update(fa)
        if (i + 1) % 100 == 0:
            print(f"rebuild {i+1}/{len(fixtures)}", flush=True)

    header = (
        "original_fixture\tmutation_kind\tmutant_path\tomega_verdict\t"
        "route_current_verdict\troute_verdict\troute_reason\tseconds\tcandidate\n"
    )
    TSV_PATH.write_text("# pin=" + PIN + f" shard={SHARD}\n" + header)
    with TSV_PATH.open("a") as f:
        for r in out_rows:
            f.write("\t".join(r) + "\n")

    PROGRESS_PATH.write_text("\n".join(f.relative_to(REPO).as_posix() for f in fixtures) + "\n")
    gen_post = sum(len(mutations_for(f.read_text())) for f in fixtures)
    stats = OUT_ROOT = TSV_PATH.parent / f"shard-{SHARD}.counts"
    stats.write_text(
        f"generated_post_dedupe={gen_post}\ntsv_rows={len(out_rows)}\nfixtures={len(fixtures)}\n"
    )
    print(f"rows={len(out_rows)} fixtures={len(fixtures)} backfilled={acct['fixture_backfilled']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
