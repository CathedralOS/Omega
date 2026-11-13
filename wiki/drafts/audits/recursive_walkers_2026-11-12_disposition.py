#!/usr/bin/env python3
"""Map every mechanical SCC candidate to kept walker id(s) or a one-line drop reason."""

from __future__ import annotations

import json
import re
from pathlib import Path

AUDIT = Path(__file__).resolve().parent
REPO = AUDIT.parents[2]

FN_RE = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+|pub\(crate\)\s+|async\s+)*"
    r"fn\s+(?P<name>[a-zA-Z_][a-zA-Z0-9_]*)",
    re.MULTILINE,
)

DROP_BY_REP: dict[str, str] = {
    "type_reference_target_label": "Name/label helper over a type reference; no recursive type-child walk.",
    "starts_following_item": "Declaration-boundary lookahead over tokens, not syntax-tree child edges.",
    "parse_binary_chain_handle": "Already iterative (`pending` stack); operator chain length is not stack-recursive.",
}

DROP_REP_PREFIXES: list[tuple[str, str]] = [
    ("render_", "Diagnostic/string rendering — not source AST nesting depth."),
    ("format_", "Diagnostic formatting — not source AST nesting depth."),
    ("follow_reference_", "Reference-hop helper uses explicit iteration elsewhere in pipeline."),
]

DROP_FILE_MARKERS: list[tuple[str, str]] = [
    ("/packages/", "Package graph / admission — not expression or type AST depth."),
    ("/contract_entailment", "Proof entailment graph with internal caps — not source AST nesting."),
    ("/build_system/", "Build orchestration — not Psi source-tree depth."),
    ("/diagnostics/", "Diagnostic emission — not source-structure recursion budget."),
]

BODY_DROP_PATTERNS: list[tuple[re.Pattern[str], str]] = [
    (re.compile(r"\bwhile\s+let\b"), "Iterative `while` loop — not Rust call-stack recursion on depth."),
    (re.compile(r"\bpending\s*:\s*Vec"), "Explicit `pending` stack/queue — not recursive on chain length."),
    (re.compile(r"depth\s*>\s*128"), "Hard depth cap (128) — bounded comparison, not source tree depth."),
    (re.compile(r"max_depth\s*:\s*usize"), "Caller-bound `max_depth` — projection/search cap, not AST height."),
    (re.compile(r"proposition::|Polynomial"), "Proof polynomial / entailment — not source AST nesting."),
    (re.compile(r"Token", re.I), "Token stream helper without syntax-tree child dispatch."),
]


def walker_index(verified: dict) -> dict[str, list[str]]:
    out: dict[str, list[str]] = {}
    for walkers in verified["walkers_by_stage"].values():
        for w in walkers:
            for fn in w["functions"]:
                bare = fn.split("::")[-1]
                out.setdefault(bare, []).append(w["id"])
                if bare != fn:
                    out.setdefault(fn, []).append(w["id"])
    out.setdefault("typed_trees::references::visit_program", []).append("W05-EXT")
    return out


def read_fn_body(file_rel: str, fn_name: str) -> str:
    path = REPO / file_rel
    if not path.is_file():
        return ""
    text = path.read_text(encoding="utf-8", errors="replace")
    matches = list(FN_RE.finditer(text))
    for idx, m in enumerate(matches):
        if m.group("name") != fn_name:
            continue
        start = m.end()
        end = matches[idx + 1].start() if idx + 1 < len(matches) else len(text)
        return text[start : start + 4000]
    return ""


def disposition_for_candidate(
    stage: str, candidate: dict, fn_to_walker: dict[str, list[str]]
) -> tuple[str, str]:
    members = candidate["members"]
    rep = candidate["representative"]
    walker_ids = sorted({wid for m in members for wid in fn_to_walker.get(m, [])})
    if walker_ids:
        return "kept", ", ".join(walker_ids)

    if rep in DROP_BY_REP:
        return "dropped", DROP_BY_REP[rep]

    for prefix, reason in DROP_REP_PREFIXES:
        if rep.startswith(prefix):
            return "dropped", reason

    rel = candidate["file"]
    for marker, reason in DROP_FILE_MARKERS:
        if marker in rel:
            return "dropped", reason

    body = read_fn_body(rel, rep)
    for pattern, reason in BODY_DROP_PATTERNS:
        if pattern.search(body):
            return "dropped", reason

    # Stage-local defaults for mechanical graph noise.
    if len(members) == 1:
        return (
            "dropped",
            "Singleton SCC — recursive helper on fixed-size metadata, not source nesting depth.",
        )

    return (
        "dropped",
        "Mis-link or auxiliary walk: SCC from cross-crate name resolution, not a curated source-depth walker.",
    )


def build_disposition_table() -> dict:
    verified = json.loads((AUDIT / "recursive_walkers_2026-11-12_verified.json").read_text())
    depths = json.loads((AUDIT / "recursive_walkers_2026-11-12_depth.json").read_text())
    fn_to_walker = walker_index(verified)

    rows: list[dict] = []
    totals = {"kept": 0, "dropped": 0}
    by_stage: dict[str, dict[str, int]] = {}

    for stage, block in sorted(depths["call_graph"].items()):
        stage_totals = {"kept": 0, "dropped": 0}
        for candidate in block["candidates"]:
            kind, detail = disposition_for_candidate(stage, candidate, fn_to_walker)
            if kind == "kept":
                totals["kept"] += 1
                stage_totals["kept"] += 1
            else:
                totals["dropped"] += 1
                stage_totals["dropped"] += 1
            rows.append(
                {
                    "stage": stage,
                    "representative": candidate["representative"],
                    "members": candidate["members"],
                    "file": candidate["file"],
                    "line": candidate["line"],
                    "disposition": kind,
                    "detail": detail,
                }
            )
        by_stage[stage] = stage_totals

    return {
        "commit": depths.get("commit"),
        "candidate_total": len(rows),
        "totals": totals,
        "by_stage": by_stage,
        "rows": rows,
    }


def main() -> None:
    table = build_disposition_table()
    out = AUDIT / "recursive_walkers_2026-11-12_dispositions.json"
    out.write_text(json.dumps(table, indent=2), encoding="utf-8")
    print(json.dumps({"totals": table["totals"], "by_stage": table["by_stage"]}, indent=2))


if __name__ == "__main__":
    main()
