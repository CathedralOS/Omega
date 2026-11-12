#!/usr/bin/env python3
"""Capture end-to-end evidence for five kept rows."""

from __future__ import annotations

import argparse
import os
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
ROUTE = REPO / "target/debug/examples/route_one_comparison"
OMEGA = REPO / "target/debug/omega"


def capture(main: Path) -> str:
    env = os.environ.copy()
    env["OMEGA_LIBRARY_ROOT"] = str(REPO / "source/library")
    env["OMEGA_CORPUS_ROOT"] = str(REPO)
    blocks: list[str] = []
    for label, cmd in (
        ("omega", [str(OMEGA), "--check", str(main)]),
        ("route", [str(ROUTE), str(main)]),
    ):
        p = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=120,
            env=env,
            stdin=subprocess.DEVNULL,
        )
        blocks.append(
            f"**{label}** exit={p.returncode}\n"
            f"```\n{(p.stdout + p.stderr)[:1200]}\n```"
        )
    return "\n".join(blocks)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, default=0)
    ap.add_argument("--count", type=int, default=5)
    args = ap.parse_args()
    tsv = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.tsv"
    picked: list[tuple[str, ...]] = []
    for line in tsv.read_text().splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) >= 6 and parts[5] == "confirmed-invalid" and parts[2]:
            picked.append(tuple(parts))
        if len(picked) >= args.count:
            break
    out: list[str] = ["## Hand verification (5 rows)", ""]
    for parts in picked:
        rel, kind, mrel, current, route, hand, *_ = parts
        mutant = REPO / mrel
        original = REPO / rel
        out += [
            f"### {rel} — {kind}",
            "",
            f"- Buckets: `{hand}`; current=`{current}`; route=`{route}`",
            f"- Obligation: `{parts[8] if len(parts) > 8 else ''}`",
            "",
            "**Diff (first changed `let` / borrow line)**",
            "```diff",
        ]
        if original.exists() and mutant.exists():
            o = original.read_text().splitlines()
            m = mutant.read_text().splitlines()
            for i, (a, b) in enumerate(zip(o, m)):
                if a != b:
                    out.append(f"- {a}")
                    out.append(f"+ {b}")
                    break
        out += ["```", ""]
        if mutant.exists():
            out.append(capture(mutant))
        out.append("")
    md_path = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.md"
    text = md_path.read_text(encoding="utf-8")
    if "## Hand verification" in text:
        text = text.split("## Hand verification")[0].rstrip() + "\n\n"
    md_path.write_text(text + "\n".join(out), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
