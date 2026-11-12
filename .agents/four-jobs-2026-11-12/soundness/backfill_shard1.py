#!/usr/bin/env python3
"""Import prior single-mutant rows/checkpoints so multi-kind expansion skips re-runs."""
from __future__ import annotations

import csv
import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE))
import run_shard1 as r1

REPO = r1.REPO
BAK = _HERE / "shard-1.tsv.bak"
TSV = r1.TSV
CHECKPOINT = r1.CHECKPOINT

CANON = {"amp_to_ampmut": "mut_ref_instead_of_shared"}


def canon(kind: str) -> str:
    return CANON.get(kind, kind)


def load_done() -> set[str]:
    if not CHECKPOINT.exists():
        return set()
    return {ln.strip() for ln in CHECKPOINT.read_text().splitlines() if ln.strip()}


def existing_pairs(tsv_path: Path) -> set[tuple[str, str]]:
    out: set[tuple[str, str]] = set()
    if not tsv_path.is_file():
        return out
    with tsv_path.open(newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f, delimiter="\t"):
            out.add((row["fixture"], canon(row.get("mutation_kind", ""))))
    return out


def drop_requires_site(fixture: str, kind: str) -> str:
    if kind != "drop_requires":
        return ""
    src = (REPO / fixture).read_text(encoding="utf-8")
    muts = [t for t in r1.mutators(src) if t[0] == "drop_requires"]
    return muts[0][2] if muts else ""


def main() -> None:
    if not BAK.is_file():
        print("no shard-1.tsv.bak; skip")
        return
    done = load_done()
    pairs = existing_pairs(TSV)
    hdr = None
    with BAK.open(newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        hdr = list(reader.fieldnames or [])
        bak_rows = list(reader)

    write_hdr = not TSV.exists() or TSV.stat().st_size == 0
    out_hdr = [
        "fixture",
        "tier",
        "mutation_kind",
        "mutant_site",
        "mutant_path",
        "current_verdict",
        "route_verdict",
        "route_reason",
        "agree",
        "hand_verdict",
        "verdict_reason",
        "spec_file",
        "spec_quote",
        "obligation_line",
        "current_out",
        "route_out",
        "current_s",
        "route_s",
    ]
    imported = 0
    with TSV.open("a" if not write_hdr else "w", encoding="utf-8") as tsv:
        if write_hdr:
            tsv.write("\t".join(out_hdr) + "\n")
        for row in bak_rows:
            fixture = row["fixture"]
            kind = canon(row.get("mutation_kind", ""))
            key = f"{fixture}\t{kind}" if kind != "none" else f"{fixture}\tnone"
            if key in done or (fixture, kind) in pairs:
                continue
            mp = row.get("mutant_path", "")
            if kind != "none" and mp and not (REPO / mp).is_dir():
                continue
            site = drop_requires_site(fixture, kind) if kind == "drop_requires" else ""
            tsv.write(
                "\t".join(
                    [
                        fixture,
                        row.get("tier", r1.tier_of(fixture)),
                        kind,
                        site,
                        mp,
                        row.get("current_verdict", ""),
                        row.get("route_verdict", ""),
                        row.get("route_reason", ""),
                        row.get("agree", ""),
                        "pending-triage",
                        "",
                        "",
                        "",
                        "",
                        (row.get("current_out") or "")[:800],
                        (row.get("route_out") or "")[:1200],
                        row.get("current_s", "0"),
                        row.get("route_s", "0"),
                    ]
                )
                + "\n"
            )
            with CHECKPOINT.open("a") as ck:
                ck.write(key + "\n")
            imported += 1
    print("imported", imported, "rows; checkpoint", len(load_done()))


if __name__ == "__main__":
    main()
