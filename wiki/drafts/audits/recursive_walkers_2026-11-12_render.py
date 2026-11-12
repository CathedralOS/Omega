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
    dispositions_path = AUDIT / "recursive_walkers_2026-11-12_dispositions.json"
    dispositions = (
        json.loads(dispositions_path.read_text())
        if dispositions_path.is_file()
        else None
    )
    d = depths["depths"]
    parser = d.get("parser")
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
        f"Lexically scanned **{d['file_count']}** `.omg` files under `tests/omega`,",
        "`samples`, `source/library`, and `source/psi` for parenthesis tests and",
        "prefix chains. **Type nesting** and **binary operator chains** use the",
        "real Psi stage-01 parser (`tokens-to-syntax-trees`) over every parsed file",
        "(see `wiki/drafts/audits/measure_depths/`). Expression **tree depth** is the",
        "maximum parse-tree height (parenthesis proxy).",
        "",
        "**Deliberate depth-limit fixtures** (excluded from “real program” column): "
        + ", ".join(f"`{p}`" for p in d["fixture_paths"])
        + ". `pass/expressions/deep_paren_nesting` is annotated "
        "`// Deep parenthesis nesting within the compile path's recursion budget.`",
        "",
    ]
    if parser:
        pa = parser["all"]
        pe = parser["excluding_depth_fixtures"]
        lines += [
            "| Dimension | All corpus (max) | Excluding depth fixtures (max) | Deepest real program (file:line) |",
            "| --- | ---: | ---: | --- |",
            f"| Expression tree depth (parse) | {pa['expression_tree_depth']['depth']} "
            f"(`{pa['expression_tree_depth']['file']}`) | "
            f"{pe['expression_tree_depth']['depth']} "
            f"(`{pe['expression_tree_depth']['file']}`) | "
            f"`{pe['expression_tree_depth']['file']}`:{pe['expression_tree_depth']['line']} |",
            f"| Type nesting (parse) | {pa['type_reference_nesting']['depth']} "
            f"(`{pa['type_reference_nesting']['file']}`) | "
            f"{pe['type_reference_nesting']['depth']} "
            f"(`{pe['type_reference_nesting']['file']}`) | "
            f"`{pe['type_reference_nesting']['file']}`:{pe['type_reference_nesting']['line']} |",
            f"| Binary op chain (parse, left spine) | {pa['binary_operator_chain']['depth']} "
            f"(`{pa['binary_operator_chain']['file']}`:{pa['binary_operator_chain']['line']}) | "
            f"{pe['binary_operator_chain']['depth']} "
            f"(`{pe['binary_operator_chain']['file']}`:{pe['binary_operator_chain']['line']}) | "
            f"`{pe['binary_operator_chain']['file']}`:{pe['binary_operator_chain']['line']} |",
        ]
        lex_paren = d["excluding_depth_fixtures"]["max"]["paren"]
        lex_paren_f = d["excluding_depth_fixtures"]["file"]["paren"]
        lines.append(
            f"| Lexical `(` depth (sanity / fixtures) | {d['all']['max']['paren']} "
            f"(`{d['all']['file']['paren']}`) | {lex_paren} (`{lex_paren_f}`) | "
            f"`{lex_paren_f}` (parser uses tree depth above) |"
        )
        lines.append(
            f"| Prefix `~` chain (lexical, in parens) | "
            f"{d['all']['max']['prefix_chain']} | "
            f"{d['excluding_depth_fixtures']['max']['prefix_chain']} | "
            f"`{d['excluding_depth_fixtures']['file']['prefix_chain']}` |"
        )
        lines += [
            "",
            f"Parser pass rate: **{parser['parsed_files']}** parsed, "
            f"**{parser['parse_failures']}** failures "
            "(some stress fixtures need larger host stack to parse).",
        ]
    else:
        lines.append("_Parser depths missing — run `measure_depths` then `analyze.py`._")
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

    if dispositions:
        lines += [
            "",
            "## Appendix B — Mechanical SCC candidate disposition",
            "",
            f"Cross-crate Tarjan SCCs flagged `keep:review` by `analyze.py`: "
            f"**{dispositions['candidate_total']}** candidates "
            f"(**{dispositions['totals']['kept']}** map to curated walker IDs, "
            f"**{dispositions['totals']['dropped']}** dropped).",
            "",
            "| Stage | Candidates | Kept | Dropped |",
            "| --- | ---: | ---: | ---: |",
        ]
        for stage in sorted(dispositions["by_stage"]):
            t = dispositions["by_stage"][stage]
            n = t["kept"] + t["dropped"]
            lines.append(f"| {stage} | {n} | {t['kept']} | {t['dropped']} |")
        lines += [
            "",
            "| Stage | Representative | Disposition | Detail |",
            "| --- | --- | --- | --- |",
        ]
        for row in dispositions["rows"]:
            disp = (
                f"kept ({row['detail']})"
                if row["disposition"] == "kept"
                else f"dropped — {row['detail']}"
            )
            lines.append(
                f"| {row['stage']} | `{row['representative']}` | {disp} | "
                f"`{row['file']}`:{row['line']} |"
            )

    lines += [
        "",
        "## Appendix C — Limits and unverified edges",
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
        "mbx build --manifest-path wiki/drafts/audits/measure_depths/Cargo.toml",
        "ulimit -s unlimited",
        "wiki/drafts/audits/measure_depths/target/debug/measure-depths . \\",
        "  > wiki/drafts/audits/recursive_walkers_2026-11-12_parser_depths.json",
        "python3 wiki/drafts/audits/recursive_walkers_2026-11-12_analyze.py",
        "python3 wiki/drafts/audits/recursive_walkers_2026-11-12_disposition.py",
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
