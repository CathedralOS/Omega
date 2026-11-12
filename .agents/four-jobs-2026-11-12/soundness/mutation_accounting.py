"""Per-fixture mutation generation accounting (generated vs TSV rows)."""

from __future__ import annotations

import hashlib
import re
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

from run_shard_mutations import Mutation, mutations_for, shard_fixtures

REPO = Path(__file__).resolve().parents[3]


def _mutations_pre_dedupe(text: str) -> list[tuple[str, str]]:
    """Duplicate mutations_for internal adds before hash dedupe."""
    from run_shard_mutations import (
        ENSURES_RE,
        LET_MUT_RE,
        REF_MUT_RE,
        REF_SHARED_RE,
        REQUIRES_RE,
    )

    out: list[tuple[str, str]] = []

    def add(kind: str, new: str) -> None:
        if new != text:
            out.append((kind, hashlib.sha256(new.encode()).hexdigest()[:16]))

    m = REQUIRES_RE.search(text)
    if m:
        lines = text.splitlines(keepends=True)
        for i, ln in enumerate(lines):
            if REQUIRES_RE.match(ln):
                add("drop_requires", "".join(lines[:i] + lines[i + 1 :]))
                break
    if m:
        body = m.group(3)
        widened = body
        for a, b in [(">=", "<"), (">", "<="), ("<=", ">"), ("<", ">="), ("==", "!=")]:
            if a in body:
                widened = body.replace(a, b, 1)
                break
        if widened != body:
            new = text[: m.start(3)] + widened + text[m.end(3) :]
            add("widen_requires", new)
    em = ENSURES_RE.search(text)
    if em:
        body = em.group(3)
        if "==" in body:
            weakened = body.replace("==", "!=", 1)
            new = text[: em.start(3)] + weakened + text[em.end(3) :]
            add("break_ensures", new)
        lines = text.splitlines(keepends=True)
        for i, ln in enumerate(lines):
            if ENSURES_RE.match(ln):
                add("drop_ensures", "".join(lines[:i] + lines[i + 1 :]))
                break
    if LET_MUT_RE.search(text):
        add("remove_mut", LET_MUT_RE.sub("let ", text, count=1))
    if REF_MUT_RE.search(text):
        add("shared_instead_of_mut", REF_MUT_RE.sub("&", text, count=1))
    if REF_SHARED_RE.search(text) and ("=" in text or ".value" in text):
        add("mut_ref_instead_of_shared", REF_SHARED_RE.sub("&mut ", text, count=1))
    guard = re.compile(r"^\s*if\s+.+\{\s*$", re.MULTILINE)
    gm = guard.search(text)
    if gm and re.search(r"/\s|/\w|\[\s*\w", text):
        lines = text.splitlines(keepends=True)
        for i, ln in enumerate(lines):
            if guard.match(ln):
                add("drop_guard", "".join(lines[:i] + lines[i + 1 :]))
                break
    return out


@dataclass
class ShardAccounting:
    fixtures: int
    generated_pre_dedupe: int
    generated_post_dedupe: int
    source_hash_collapsed: int
    tsv_rows: int
    no_mutation_rows: int
    non_candidate_rows: int
    fixture_gap_rows: int


def reconcile_tsv(rows: list[list[str]], fixtures: list[Path]) -> tuple[ShardAccounting, Counter[str]]:
    """Explain generated_post_dedupe - tsv_rows."""
    reasons: Counter[str] = Counter()
    pre = post = 0
    for f in fixtures:
        text = f.read_text()
        pre_list = _mutations_pre_dedupe(text)
        post_list = mutations_for(text)
        pre += len(pre_list)
        post += len(post_list)
    reasons["source_hash_collapsed"] = pre - post

    tsv_rows = len(rows)
    no_mut = sum(1 for r in rows if len(r) > 1 and r[1] == "no-mutation")
    non_cand = sum(1 for r in rows if len(r) > 8 and r[8] == "no")
    by_fix = {f.relative_to(REPO).as_posix() for f in fixtures}
    row_fix = {r[0] for r in rows if r[0].startswith("tests/")}
    reasons["fixtures_without_any_row"] = len(by_fix - row_fix)

    expected_rows = 0
    for f in fixtures:
        if mutations_for(f.read_text()):
            expected_rows += len(mutations_for(f.read_text()))
        else:
            expected_rows += 1
    acc = ShardAccounting(
        fixtures=len(fixtures),
        generated_pre_dedupe=pre,
        generated_post_dedupe=post,
        source_hash_collapsed=pre - post,
        tsv_rows=tsv_rows,
        no_mutation_rows=no_mut,
        non_candidate_rows=non_cand,
        fixture_gap_rows=reasons["fixtures_without_any_row"],
    )
    reasons["expected_tsv_rows_after_full_emit"] = expected_rows
    reasons["fixtures_missing_all_rows"] = reasons["fixtures_without_any_row"]
    gap = expected_rows - tsv_rows
    if gap > 0:
        reasons["missing_rows_vs_expected_emit"] = gap
    elif gap < 0:
        reasons["extra_rows_vs_expected_emit"] = -gap
    return acc, reasons


def load_runner_rows(tsv_path: Path) -> list[list[str]]:
    rows: list[list[str]] = []
    for line in tsv_path.read_text().splitlines():
        if not line or line.startswith("#") or line.startswith("original_fixture"):
            continue
        p = line.split("\t")
        if not p[0].startswith("tests/"):
            continue
        if len(p) < 9:
            continue
        rows.append(p[:9])
    return rows
