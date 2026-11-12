#!/usr/bin/env python3
"""Turn TSV candidates into shard markdown with wiki/spec quotes by mutation kind."""

from __future__ import annotations

import re
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SHARD = 7
OUT_ROOT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT_ROOT / f"shard-{SHARD}.tsv"
MD = OUT_ROOT / f"shard-{SHARD}.md"

# Mutation kind -> (spec path relative to wiki/spec, excerpt line search keywords)
RULE_HINTS: dict[str, tuple[str, list[str]]] = {
    "drop_requires": ("language/contracts.md", ["requires", "precondition", "obligation"]),
    "widen_requires": ("language/contracts.md", ["requires", "precondition"]),
    "drop_ensures": ("language/contracts.md", ["ensures", "postcondition"]),
    "break_ensures": ("language/contracts.md", ["ensures", "postcondition"]),
    "drop_mut": ("language/ownership.md", ["mut", "mutable", "Owned local mutability"]),
    "shared_instead_of_mut": ("language/ownership.md", ["&mut", "mutable borrow", "write"]),
    "mut_ref_instead_of_shared": ("language/ownership.md", ["&mut", "exclusive", "shared"]),
    "drop_guard": ("language/expressions.md", ["division", "index", "bounds", "guard"]),
}


def load_spec_excerpt(rel: str, keywords: list[str]) -> str | None:
    path = REPO / "wiki/spec" / rel
    if not path.exists():
        for p in (REPO / "wiki/spec").rglob("*.md"):
            if p.name == Path(rel).name:
                path = p
                break
    if not path.exists():
        return None
    text = path.read_text()
    lines = text.splitlines()
    best = None
    for i, line in enumerate(lines):
        low = line.lower()
        if any(k.lower() in low for k in keywords):
            start = max(0, i - 1)
            end = min(len(lines), i + 3)
            chunk = "\n".join(lines[start:end]).strip()
            if best is None or len(chunk) > len(best):
                best = chunk
    return best


def main() -> None:
    if not TSV.exists():
        MD.write_text(f"# Soundness mutations shard {SHARD}\n\n(no data yet)\n")
        return

    rows = []
    for line in TSV.read_text().splitlines():
        if not line or line.startswith("#") or line.startswith("original_fixture"):
            continue
        parts = line.split("\t")
        if len(parts) < 9:
            continue
        if parts[-1] != "yes":
            continue
        rows.append(parts)

    kept_holes = []
    disagreements = []
    discarded_valid = 0
    discarded_no_rule = 0

    for parts in rows:
        (
            orig,
            kind,
            mutant,
            omg,
            cur,
            route,
            secs,
            _cand,
        ) = parts[:8]
        both_accept = omg == "accepted" or cur == "accepted" or route == "accepted"
        disagree = (
            omg in ("accepted", "rejected")
            and cur in ("accepted", "rejected")
            and omg != cur
        ) or (
            cur in ("accepted", "rejected")
            and route in ("accepted", "rejected", "incomplete", "fallback")
            and cur != route
        )

        if both_accept and omg == "rejected" and cur == "rejected" and route == "rejected":
            continue

        hint = RULE_HINTS.get(kind)
        quote = None
        spec_path = None
        if hint:
            spec_path, kws = hint
            quote = load_spec_excerpt(spec_path, kws)

        if both_accept and not (omg == "rejected" and cur == "rejected"):
            if not quote:
                discarded_no_rule += 1
                continue
            # Heuristic: if all reject, not a hole
            if omg == "rejected" and cur == "rejected" and route in (
                "rejected",
                "incomplete",
                "fallback",
            ):
                continue
            if omg == "accepted" or cur == "accepted" or route == "accepted":
                kept_holes.append((parts, spec_path, quote))
        elif disagree:
            disagreements.append((parts, spec_path, quote))

    generated = run = kept = 0
    counts_file = OUT_ROOT / f"shard-{SHARD}.counts"
    if counts_file.exists():
        for ln in counts_file.read_text().splitlines():
            if "=" in ln:
                k, v = ln.split("=", 1)
                if k == "generated":
                    generated = int(v)
                elif k == "run":
                    run = int(v)
                elif k == "candidate":
                    pass

    lines = [
        f"# Soundness mutations shard {SHARD} (job 2)",
        "",
        "PIN: `0835b3aaf803ed97f905be42d94727ff58459263`",
        "",
        "## Commands",
        "",
        "```bash",
        "cd /agent/repos/omega",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison",
        "FOUR_JOBS_SHARD=7 python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py",
        "python3 .agents/four-jobs-2026-11-12/soundness/review_shard_candidates.py",
        "```",
        "",
        "Per mutant: `timeout 120 omega --check <mutant>/main.omg` and",
        "`timeout 120 target/debug/examples/route_one_comparison <mutant>/main.omg`.",
        "",
        "## Counts",
        "",
        f"- mutants generated (this run): {generated}",
        f"- mutants run: {run}",
        f"- candidates flagged in TSV: {len(rows)}",
        f"- confirmed holes in report: {len(kept_holes)}",
        f"- checker disagreements: {len(disagreements)}",
        f"- discarded (no spec quote): {discarded_no_rule}",
        "",
        "## Confirmed holes",
        "",
    ]

    for parts, spec_path, quote in kept_holes:
        orig, kind, mutant, omg, cur, route, secs, _ = parts[:8]
        src = (REPO / mutant).read_text()
        lines.extend(
            [
                f"### {kind} — `{orig}`",
                "",
                f"- Mutant: `{mutant}`",
                f"- omega: **{omg}**; route current: **{cur}**; route: **{route}**",
                f"- Spec: `wiki/spec/{spec_path}`",
                "",
                "> " + quote.replace("\n", "\n> ") if quote else "(missing quote)",
                "",
                "```omg",
                src.strip(),
                "```",
                "",
            ]
        )

    lines.extend(["## Disagreements", ""])
    for parts, spec_path, quote in disagreements:
        orig, kind, mutant, omg, cur, route, secs, _ = parts[:8]
        lines.extend(
            [
                f"### {kind} — `{orig}`",
                "",
                f"- Mutant: `{mutant}`",
                f"- omega: **{omg}**; route current: **{cur}**; route: **{route}**",
                "",
            ]
        )
        if quote and spec_path:
            lines.extend(
                [
                    f"- Spec (invalidity): `wiki/spec/{spec_path}`",
                    "",
                    "> " + quote.replace("\n", "\n> "),
                    "",
                ]
            )

    MD.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
