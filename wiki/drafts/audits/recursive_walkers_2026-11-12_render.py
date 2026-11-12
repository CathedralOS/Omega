#!/usr/bin/env python3
"""Render recursive_walkers_2026-11-12.md from verified inventory + depth JSON."""

from __future__ import annotations

import json
from pathlib import Path

AUDIT = Path(__file__).resolve().parent
REPO = AUDIT.parents[2]


def main() -> None:
    verified = json.loads((AUDIT / "recursive_walkers_2026-11-12_verified.json").read_text())
    depths = json.loads((AUDIT / "recursive_walkers_2026-11-12_depth.json").read_text())
    d = depths["depths"]
    lines: list[str] = []
    lines += [
        "# Recursive walker inventory (Psi stages 01–06)",
        "",
        "Curated, hand-verified inventory at `0835b3aaf803ed97f905be42d94727ff58459263`",
        "for [expression-tree-depth-provision](OWNER_QUESTIONS.md). Lists only walkers",
        "whose **Rust stack recursion** scales with **source** expression/type/pattern/",
        "statement nesting or operator/projection chain length. Stage **05** spans",
        "`typed-trees-to-checked-trees` and `validation`; stages **07–08** are out of scope.",
        "",
        "## Shared flat-table groups (one table retires the whole group)",
        "",
    ]
    for g in verified["groups"]:
        wids = ", ".join(f"`{w}`" for w in g["walker_ids"])
        lines.append(f"### `{g['id']}` — {g['title']}")
        lines.append("")
        lines.append(f"- **Flat representation:** {g['flat_table']}")
        lines.append(f"- **Stages:** {g['stages']}")
        lines.append(f"- **Walker IDs:** {wids}")
        lines.append(f"- **Notes:** {g['notes']}")
        lines.append("")

    lines += [
        "## Measured source depths",
        "",
        f"Scanned **{d['file_count']}** `.omg` files under `tests/omega`, `samples`,",
        "`source/library`, and `source/psi`. Parenthesis and angle counts are",
        "delimiter scans with comments/strings stripped. **Binary operator chain**",
        "counts the longest run of binary punctuators inside a single `(...)`",
        "region (parse-lexical; see limitations). **Prefix chain** counts",
        "consecutive `~` / `&` / `*` / `!` at operand starts inside parens.",
        "",
        "Excluded depth-test fixtures: "
        + ", ".join(f"`{p}`" for p in d["fixture_paths"])
        + ".",
        "",
        "| Dimension | All corpus (max) | Excluding depth fixtures (max) | Deepest real program |",
        "| --- | ---: | ---: | --- |",
    ]
    labels = {
        "paren": "Parenthesis nesting",
        "angle": "Type `<…>` nesting",
        "binary_chain": "Binary op chain (in one paren group)",
        "prefix_chain": "Prefix operator chain",
    }
    for key, label in labels.items():
        all_m = d["all"]["max"][key]
        ex_m = d["excluding_depth_fixtures"]["max"][key]
        ex_f = d["excluding_depth_fixtures"]["file"][key]
        all_f = d["all"]["file"][key]
        lines.append(
            f"| {label} | {all_m} (`{all_f}`) | {ex_m} (`{ex_f}`) | `{ex_f}` |"
        )
    pins = d["parser_test_pins"]
    lines += [
        "",
        "**Stage-01 integration tests** (Rust, not `.omg`): "
        f"{pins['unary_tilde_repeat']} unary `~` repetitions; "
        f"{pins['binary_seven_op_groups']} seven-operator groups "
        "(binary chains use `parse_binary_chain_handle` with an explicit "
        "`pending` stack — no Rust recursion on chain length).",
        "",
        "## Kept walker counts (verified)",
        "",
        "| Stage | Kept walkers |",
        "| --- | ---: |",
    ]
    for stage in sorted(verified["counts_by_stage"]):
        lines.append(f"| {stage} | {verified['counts_by_stage'][stage]} |")
    lines.append(f"| **Total** | **{verified['total_kept']}** |")
    lines.append(
        f"| **Shared-tree groups** | **{len(verified['groups'])}** |"
    )
    lines.append("")

    for stage in sorted(verified["walkers_by_stage"]):
        lines.append(f"## {stage}")
        lines.append("")
        lines.append(
            "| ID | Function(s) | Location | Walks | Depth driver | Existing flat table |"
        )
        lines.append("| --- | --- | --- | --- | --- | --- |")
        for w in verified["walkers_by_stage"][stage]:
            funcs = ", ".join(f"`{f}`" for f in w["functions"])
            lines.append(
                f"| `{w['id']}` | {funcs} | `{w['location']}` | {w['walks']} | "
                f"{w['depth_driver']} | {w['flat_table']} |"
            )
        lines.append("")

    lines += [
        "## Appendix A — Recursion dropped (not source-depth-driven)",
        "",
        "| ID | Representative | Reason |",
        "| --- | --- | --- |",
    ]
    for row in verified["dropped"]:
        lines.append(f"| `{row['id']}` | `{row['representative']}` | {row['reason']} |")

    lines += [
        "",
        "## Appendix B — Limits and unverified edges",
        "",
    ]
    for note in verified["limitations"]:
        lines.append(f"- {note}")
    lines.append("")
    lines += [
        "## Method",
        "",
        "```bash",
        "git checkout 0835b3aaf803ed97f905be42d94727ff58459263",
        "python3 wiki/drafts/audits/recursive_walkers_2026-11-12_analyze.py",
        "python3 wiki/drafts/audits/recursive_walkers_2026-11-12_render.py",
        "```",
        "",
        "- **Call graph (appendix input):** stage-local union of stage crate `src/`",
        "  trees; callee resolution by function name across crates in the same stage",
        "  (may miss `mod` paths and trait calls). Curated list above overrides.",
        "- **Depths:** `recursive_walkers_2026-11-12_analyze.py`.",
        "- **Report body:** `recursive_walkers_2026-11-12_verified.json` (hand-verified).",
        "",
        "## Host",
        "",
        "Cursor Cloud Agent VM, Linux, `/agent/repos/omega`.",
        "",
    ]
    (AUDIT / "recursive_walkers_2026-11-12.md").write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    main()
