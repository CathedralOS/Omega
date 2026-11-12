#!/usr/bin/env python3
"""Job 2 soundness mutation runner (lead-review buckets, shard S of 8)."""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

from q7_gaps import load_q7_gap_sites

PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = int(os.environ.get("SOUNDNESS_SHARD", "6"))
SHARD_COUNT = 8
REPO = Path(__file__).resolve().parents[3]
OUT_ROOT = REPO / ".agents" / "four-jobs-2026-11-12" / "soundness"
OMEGA = REPO / "target" / "debug" / "omega"
ROUTE = REPO / "target" / "debug" / "examples" / "route_one_comparison"
CHECK_TIMEOUT = 120
ENV = {
    **os.environ,
    "OMEGA_LIBRARY_ROOT": str(REPO / "source/library"),
    "OMEGA_CORPUS_ROOT": str(REPO),
}

SPEC_RULES: dict[str, tuple[str, str, str]] = {
    "remove_mut": (
        "wiki/spec/language/expressions.md",
        "Assignment and explicit `&mut` / `&write` loans",
        "Such a loan requires existing writable authority at the selected place; "
        "owned-local mutability requires a `mut` binding when that place is the local's owned storage.",
    ),
    "weaken_requires": (
        "wiki/spec/language/machines.md",
        "Call preconditions",
        "Every machine application establishes the selected contract's `requires` under "
        "the exact argument substitution.",
    ),
    "drop_requires": (
        "wiki/spec/language/machines.md",
        "Call preconditions",
        "Every machine application establishes the selected contract's `requires` under "
        "the exact argument substitution.",
    ),
    "remove_guard": (
        "wiki/spec/language/state_contracts.md",
        "Requirement clauses",
        "Entry obligations established at every invocation under exact actual-argument "
        "substitution must be dischargeable at every reachable call.",
    ),
    "flip_to_mut_loan": (
        "wiki/spec/language/expressions.md",
        "Assignment and explicit `&mut` / `&write` loans",
        "For named machine calls, mutable and write-only lending require explicit `&mut` "
        "and `&write` expressions. Such a loan requires existing writable authority at the selected place.",
    ),
}

LET_RE = re.compile(r"^\s*let\s+(?P<mut>mut\s+)?(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b", re.MULTILINE)
BORROW_MUT_RE = re.compile(r"&\s*(mut|write)\s+(?P<root>[A-Za-z_][A-Za-z0-9_]*)\b")
STORE_RE = re.compile(
    r"^\s*(?P<root>[A-Za-z_][A-Za-z0-9_]*)(?:\[[^\]]+\]|\.[A-Za-z_][A-Za-z0-9_]*)\s*="
)


@dataclass
class Mutation:
    kind: str
    detail: str
    text: str
    meta: dict[str, str] = field(default_factory=dict)


@dataclass
class Row:
    fixture: str
    tier: str
    kind: str
    detail: str
    bucket: str
    current: str
    route: str
    route_detail: str
    discard_reason: str
    obligation: str
    rule_file: str
    rule_heading: str
    rule_quote: str
    mutant_path: str
    current_s: float
    route_s: float


ROUTE_NON_JUDGMENT = frozenset(
    {"no-entry", "unreached", "incomplete", "defect", "error", "unreached", "fallback"}
)


def list_shard_fixtures() -> list[Path]:
    listed = subprocess.check_output(
        [
            "git",
            "-C",
            str(REPO),
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
        ],
        text=True,
    )
    paths = sorted(p for p in listed.splitlines() if p.endswith("/main.omg"))
    out = [REPO / rel for i, rel in enumerate(paths) if i % SHARD_COUNT == SHARD]
    limit = os.environ.get("SOUNDNESS_FIXTURE_LIMIT")
    return out[: int(limit)] if limit else out


def read_source(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def strip_line_comment(line: str) -> str:
    if "//" in line:
        return line.split("//", 1)[0]
    return line


def count_let_bindings(text: str, name: str) -> int:
    return len(re.findall(rf"\blet\s+(?:mut\s+)?{re.escape(name)}\b", text))


def transition_restates_requires(original: str, removed: str) -> str | None:
    removed_compact = re.sub(r"\s+", "", removed)
    for m in re.finditer(r"transition\s+([^{;\n]+)", original):
        arm = m.group(1).strip()
        arm_compact = re.sub(r"\s+", "", arm)
        if len(removed_compact) > 12 and removed_compact in arm_compact:
            return f"transition arm `{arm}` restates `{removed}`"
    return None


def find_remove_mut_target(source: str) -> tuple[str, str] | None:
    for m in LET_RE.finditer(source):
        if not m.group("mut"):
            continue
        name = m.group("name")
        tail = source[m.end() :]
        if re.search(
            rf"(?:^|\n)[^\n]*\b{re.escape(name)}\s*[\[.]"
            rf"|&\s*(?:mut|write)\s+{re.escape(name)}\b",
            tail,
            re.MULTILINE,
        ):
            return name, m.group(0).strip()
    return None


def apply_remove_mut(source: str, name: str) -> str:
    return re.sub(rf"\blet\s+mut\s+{re.escape(name)}\b", f"let {name}", source, count=1)


def find_remove_mut_violation(mutant: str, name: str) -> str | None:
    for i, raw in enumerate(mutant.splitlines(), 1):
        line = strip_line_comment(raw).strip()
        if not line or line.startswith("//"):
            continue
        bm = BORROW_MUT_RE.search(line)
        if bm and bm.group("root") == name:
            return f"{i}: {raw.rstrip()}"
        sm = STORE_RE.match(line)
        if sm and sm.group("root") == name:
            return f"{i}: {raw.rstrip()}"
        if re.match(rf"^{re.escape(name)}\s*=", line):
            return f"{i}: {raw.rstrip()}"
    return None


def first_requires_clause(text: str) -> str | None:
    m = re.search(r"requires\s+([^\n{]+)", text)
    return m.group(1).strip() if m else None


def drop_requires_dependent_obligation(
    original: str, mutant: str, removed: str
) -> str | None:
    names = set(re.findall(r"\b[a-z][a-z0-9_]*\b", removed))
    names -= {"requires", "ensures", "and", "or", "true", "false", "machine", "transition"}
    for i, raw in enumerate(mutant.splitlines(), 1):
        line = strip_line_comment(raw)
        for n in names:
            if re.search(rf"\b{re.escape(n)}\s*\[", line):
                return f"{i}: {raw.rstrip()} (index uses `{n}` formerly guarded by `{removed}`)"
            if re.search(rf"/\s*{re.escape(n)}\b|\b{re.escape(n)}\s*/", line):
                return f"{i}: {raw.rstrip()} (division uses `{n}` formerly guarded by `{removed}`)"
            if re.search(rf"\b{re.escape(n)}\s*[-+*/%]", line) and any(
                op in removed for op in (">=", ">", "<=", "<", "!=", "==")
            ):
                return f"{i}: {raw.rstrip()} (arithmetic on `{n}` formerly guarded by `{removed}`)"
    return None


def weaken_requires_breach(original: str, mutant: str, detail: str) -> str | None:
    removed = first_requires_clause(original)
    weakened = first_requires_clause(mutant)
    if not removed or not weakened or removed == weakened:
        return None
    return drop_requires_dependent_obligation(original, mutant, removed)


def flip_to_mut_line(mutant: str) -> str | None:
    for i, raw in enumerate(mutant.splitlines(), 1):
        if re.search(r"&mut\b", raw):
            return f"{i}: {raw.rstrip()}"
    return None


def remove_guard_obligation(original: str, mutant: str) -> str | None:
    orig_when = re.search(r"^\s*when\s+(.+)$", original, re.MULTILINE)
    if not orig_when:
        return None
    guard = orig_when.group(1).strip()
    if re.search(rf"^\s*when\s+{re.escape(guard)}", mutant, re.MULTILINE):
        return None
    for i, raw in enumerate(mutant.splitlines(), 1):
        if "transition" in raw and "when" not in original[max(0, original.find(raw) - 50) :]:
            return f"{i}: {raw.rstrip()} (transition arm formerly gated by `when {guard}`)"
    return f"guard removed: when {guard}"


def generate_mutations(source: str) -> list[Mutation]:
    muts: list[Mutation] = []
    target = find_remove_mut_target(source)
    if target:
        name, _decl = target
        text = apply_remove_mut(source, name)
        muts.append(
            Mutation(
                "remove_mut",
                f"remove mut from `{name}`",
                text,
                {"binding": name},
            )
        )
    if "requires" in source:
        clause = first_requires_clause(source)
        new = re.sub(r"requires\s+[^\n{]+\n", "", source, count=1)
        if new != source and clause:
            muts.append(
                Mutation("drop_requires", "delete first requires line", new, {"removed": clause})
            )
        for pat, repl, label in (
            (r"(requires[^\n]*?)(>=)", r"\1>", "requires >= to >"),
            (r"(requires[^\n]*?)(<=)", r"\1<", "requires <= to <"),
        ):
            new2 = re.sub(pat, repl, source, count=1)
            if new2 != source:
                muts.append(Mutation("weaken_requires", label, new2, {}))
    if re.search(r"^\s*when\s+", source, re.MULTILINE):
        new = re.sub(r"^\s*when\s+[^\n]+\n", "", source, count=1, flags=re.MULTILINE)
        if new != source:
            muts.append(Mutation("remove_guard", "delete first when line", new, {}))
    if re.search(r"&(?!\s*(?:mut|write)\b)", source):
        new = re.sub(r"&(?!mut|write)(\s*)", r"&mut\1", source, count=1)
        if new != source:
            muts.append(Mutation("flip_to_mut_loan", "first shared & -> &mut", new, {}))
    return muts[:8]


def parse_route_output(stdout: str) -> tuple[str, str]:
    route_verdict = "error"
    detail = ""
    for line in stdout.splitlines():
        if line.startswith("program\t"):
            for token in line.split("\t")[2:]:
                if token.startswith("route="):
                    route_verdict = token.split("=", 1)[1]
        elif line.startswith("diagnostic\t") and not detail:
            parts = line.split("\t")
            if len(parts) >= 2:
                detail = "diagnostic: " + one_line_join(parts[4:])
        elif line.startswith("fallback\t") and not detail:
            parts = line.split("\t")
            detail = "fallback: " + one_line_join(parts[2:])
    if not detail:
        detail = f"route={route_verdict}"
    return route_verdict, detail


def one_line_join(parts: list[str]) -> str:
    return " ".join(parts).replace("\t", " ")[:400]


def run_omega(path: Path) -> tuple[str, float, str, str]:
    start = time.monotonic()
    try:
        proc = subprocess.run(
            ["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(path)],
            cwd=REPO,
            env=ENV,
            capture_output=True,
            text=True,
        )
    except subprocess.TimeoutExpired:
        return "timeout", time.monotonic() - start, "", ""
    elapsed = time.monotonic() - start
    verdict = "accepted" if proc.returncode == 0 else "rejected"
    return verdict, elapsed, proc.stdout[:500], proc.stderr[:500]


def run_route(path: Path) -> tuple[str, float, str]:
    start = time.monotonic()
    try:
        proc = subprocess.run(
            ["timeout", str(CHECK_TIMEOUT), str(ROUTE), str(path)],
            cwd=REPO,
            env=ENV,
            capture_output=True,
            text=True,
        )
    except subprocess.TimeoutExpired:
        return "timeout", time.monotonic() - start, ""
    elapsed = time.monotonic() - start
    route, _ = parse_route_output(proc.stdout)
    return route, elapsed, proc.stdout


def accepts(verdict: str) -> bool:
    return verdict == "accepted"


def classify(
    kind: str,
    original: str,
    mutant: str,
    mutation: Mutation,
    fixture_rel: str,
    current: str,
    route: str,
    route_detail: str,
    q7: set[tuple[str, str]],
) -> tuple[str, str, str, str, str, str]:
    """Return bucket, obligation, rule_file, heading, quote, discard_reason."""
    empty = ("", "", "")
    if current == "timeout" or route == "timeout":
        return "discarded_timeout", "", *empty, "checker timeout"
    if not accepts(current) and not accepts(route):
        return "discarded_both_reject", "", *empty, ""
    if not accepts(current):
        return "discarded_route_only", "", *empty, route_detail

    detail_lower = route_detail.lower()
    if kind == "drop_requires":
        removed = mutation.meta.get("removed") or first_requires_clause(original) or ""
        if route in ROUTE_NON_JUDGMENT or "unsupported" in detail_lower:
            return (
                "discarded_valid_mutant",
                "",
                *empty,
                f"route non-judgment ({route}): {route_detail}",
            )
        obligation = drop_requires_dependent_obligation(original, mutant, removed) or ""
        restated = transition_restates_requires(original, removed)
        if obligation:
            rule = SPEC_RULES["drop_requires"]
            return ("confirmed_hole", obligation, rule[0], rule[1], rule[2], route_detail)
        if restated:
            return (
                "discarded_valid_mutant",
                restated,
                *empty,
                f"requires duplicated in transition; route: {route_detail}",
            )
        return (
            "discarded_valid_mutant",
            "",
            *empty,
            route_detail if not accepts(route) else "no body/ensures obligation needs removed requires",
        )

    obligation = ""
    if kind == "remove_mut":
        name = mutation.meta.get("binding", "")
        if count_let_bindings(mutant, name) > 1:
            return (
                "discarded_valid_mutant",
                "",
                *empty,
                f"binding `{name}` shadowed by inner `let`",
            )
        obligation = find_remove_mut_violation(mutant, name) or ""
        if not obligation:
            return "discarded_valid_mutant", "", *empty, "no write/borrow on target binding"
        rule = SPEC_RULES["remove_mut"]
        if (fixture_rel, name) in q7:
            return (
                "known_gap_q7_mut",
                obligation,
                rule[0],
                rule[1],
                rule[2],
                "",
            )
    elif kind == "weaken_requires":
        obligation = weaken_requires_breach(original, mutant, mutation.detail) or ""
        if not obligation:
            return "discarded_valid_mutant", "", *empty, "weakened requires still dischargeable"
    elif kind == "remove_guard":
        obligation = remove_guard_obligation(original, mutant) or ""
        if not obligation:
            return "discarded_valid_mutant", "", *empty, "guard removal still checked"
    elif kind == "flip_to_mut_loan":
        obligation = flip_to_mut_line(mutant) or ""
        if not obligation:
            return "discarded_valid_mutant", "", *empty, "no &mut site"
    else:
        return "discarded_no_specific_rule", obligation, *empty, ""

    if not obligation:
        return "discarded_valid_mutant", "", *empty, "no spec-breaking site in mutant"
    rule = SPEC_RULES.get(kind)
    if not rule:
        return "discarded_no_specific_rule", obligation, *empty, ""
    return "confirmed_hole", obligation, rule[0], rule[1], rule[2], ""


def mutant_id(fixture: Path, kind: str, detail: str) -> str:
    h = hashlib.sha256(f"{fixture}:{kind}:{detail}".encode()).hexdigest()[:16]
    safe = fixture.relative_to(REPO).as_posix().replace("/", "__")
    return f"{safe}__{kind}__{h}"


def stage_mutant(fixture: Path, text: str, mid: str) -> Path:
    dest = OUT_ROOT / f"shard-{SHARD}" / "mutants" / mid
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(fixture.parent, dest)
    (dest / "main.omg").write_text(text, encoding="utf-8")
    return dest / "main.omg"


def bucket_counts(rows: list[Row]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for r in rows:
        counts[r.bucket] = counts.get(r.bucket, 0) + 1
    return counts


def regen_mutant_text(fixture_rel: str, kind: str, detail: str) -> tuple[str, str] | None:
    fixture = REPO / fixture_rel
    original = read_source(fixture)
    for m in generate_mutations(original):
        if m.kind == kind and m.detail == detail:
            return original, m.text
    return None


def hand_verify_row(row: Row) -> list[str]:
    fixture = REPO / row.fixture
    orig = read_source(fixture)
    if row.mutant_path:
        mutant = REPO / row.mutant_path
        mut = read_source(mutant)
    else:
        regen = regen_mutant_text(row.fixture, row.kind, row.detail)
        if not regen:
            return [f"#### Hand verification: {row.fixture} — could not regen mutant"]
        orig, mut = regen
        mid = mutant_id(fixture, row.kind, row.detail)
        mutant = stage_mutant(fixture, mut, mid)
    cur_v, _, cur_out, cur_err = run_omega(mutant)
    route_v, _, route_out = run_route(mutant)
    _, route_detail = parse_route_output(route_out)
    lines = [
        f"#### Hand verification: {row.fixture} / {row.kind}",
        "",
        f"- Bucket: `{row.bucket}`",
        f"- Obligation: `{row.obligation}`",
        "",
        "**Mutant diff (excerpt)**",
        "```diff",
    ]
    o_lines, m_lines = orig.splitlines(), mut.splitlines()
    for i, (a, b) in enumerate(zip(o_lines, m_lines)):
        if a != b:
            lines.append(f"- {a}")
            lines.append(f"+ {b}")
    if len(m_lines) != len(o_lines):
        lines.append(f"({'+' if len(m_lines) > len(o_lines) else ''}lines differ)")
    lines.extend(
        [
            "```",
            "",
            f"**`omega --check`** exit={'0' if cur_v == 'accepted' else 'nonzero'} verdict=`{cur_v}`",
            "```",
            (cur_out or cur_err or "(empty)")[:800],
            "```",
            "",
            f"**`route_one_comparison`** route=`{route_v}`",
            f"- Route detail: {route_detail}",
            "```",
            route_out[:1200] or "(empty)",
            "```",
            "",
        ]
    )
    if not row.mutant_path and mutant.parent.name.startswith("tests__"):
        shutil.rmtree(mutant.parent, ignore_errors=True)
    return lines


def write_md(
    path: Path,
    fixtures_total: int,
    originals_skipped: int,
    fixtures_zero_mutations: int,
    mutants_attempted: int,
    rows: list[Row],
    finished: bool,
) -> None:
    n_run = len(rows)
    buckets = bucket_counts(rows)
    by_kind: dict[str, int] = {}
    for r in rows:
        if r.bucket == "confirmed_hole":
            by_kind[r.kind] = by_kind.get(r.kind, 0) + 1

    lines = [
        f"# Soundness mutations shard {SHARD} (PIN {PIN})",
        "",
        f"Status: {'complete' if finished else 'in progress'}",
        "",
        "Lead review buckets (every mutant row belongs to exactly one bucket below).",
        "",
        "## Commands",
        "",
        "```bash",
        f"git checkout {PIN}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega --bin omega",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison",
        "export OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_ROOT=$PWD",
        "python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py",
        "```",
        "",
        "## Fixture coverage",
        "",
        f"| Metric | Count |",
        f"| --- | ---: |",
        f"| Fixtures in shard | {fixtures_total} |",
        f"| Originals skipped (current checker does not accept) | {originals_skipped} |",
        f"| Fixtures checked with zero mutation patterns | {fixtures_zero_mutations} |",
        f"| Mutants attempted (generated × checked) | {mutants_attempted} |",
        f"| Mutant rows recorded | {n_run} |",
        "",
        "Mutant total stability: `mutants_attempted` equals the sum of bucket counts below "
        "(one row per attempted mutant). Fixtures with no applicable patterns do not add rows.",
        "",
        "## Mutant buckets (sum = mutants attempted)",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    bucket_order = [
        "confirmed_hole",
        "known_gap_q7_mut",
        "discarded_both_reject",
        "discarded_route_only",
        "discarded_valid_mutant",
        "discarded_no_specific_rule",
        "discarded_timeout",
        "mutant_run_error",
    ]
    total = 0
    for b in bucket_order:
        c = buckets.get(b, 0)
        total += c
        lines.append(f"| `{b}` | {c} |")
    for b, c in sorted(buckets.items()):
        if b not in bucket_order:
            total += c
            lines.append(f"| `{b}` | {c} |")
    lines.append(f"| **Total** | **{total}** |")
    if finished and total != mutants_attempted:
        lines.append(
            f"\n**Bucket mismatch:** total {total} vs attempted {mutants_attempted} (investigate)."
        )

    lines.extend(
        [
            "",
            "## Confirmed holes by mutation kind",
            "",
            "| Kind | Count |",
            "| --- | ---: |",
        ]
    )
    for k, c in sorted(by_kind.items()):
        lines.append(f"| `{k}` | {c} |")
    if not by_kind:
        lines.append("| (none) | 0 |")

    kept = [r for r in rows if r.bucket in ("confirmed_hole", "known_gap_q7_mut")]
    lines.extend(["", "## Kept rows", ""])
    for r in kept:
        lines.extend(
            [
                f"### {r.fixture} — {r.kind}",
                "",
                f"- Bucket: `{r.bucket}`",
                f"- Mutation: {r.detail}",
                f"- Mutant: `{r.mutant_path}`",
                f"- Current: `{r.current}`; route: `{r.route}`",
                f"- Route detail: {r.route_detail[:300] if r.route_detail else '(n/a)'}",
                f"- Broken obligation: `{r.obligation}`",
                f"- Spec (`{r.rule_file}` § {r.rule_heading}):",
                "",
                f"> {r.rule_quote}",
                "",
            ]
        )

    samples = [r for r in rows if r.bucket == "confirmed_hole"][:5]
    if samples:
        lines.extend(["", "## Hand verification (5 confirmed holes)", ""])
        for r in samples:
            lines.extend(hand_verify_row(r))
    drop_discarded = [
        r
        for r in rows
        if r.kind == "drop_requires"
        and r.bucket == "discarded_valid_mutant"
        and accepts(r.current)
    ][:5]
    if drop_discarded:
        lines.extend(["", "## Hand verification (5 discarded drop_requires, current accepts)", ""])
        for r in drop_discarded:
            lines.extend(hand_verify_row(r))
    rm_samples = [r for r in rows if r.kind == "remove_mut"][:5]
    if rm_samples:
        lines.extend(["", "## Hand verification (5 remove_mut rows)", ""])
        for r in rm_samples:
            lines.extend(hand_verify_row(r))
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def maybe_git_push() -> None:
    if os.environ.get("SOUNDNESS_NO_PUSH") == "1":
        return
    try:
        subprocess.run(
            ["git", "add", "-A", ".agents/four-jobs-2026-11-12/soundness"],
            cwd=REPO,
            check=False,
            capture_output=True,
        )
        subprocess.run(
            ["git", "commit", "-m", f"docs: soundness shard {SHARD} progress", "--allow-empty"],
            cwd=REPO,
            check=False,
            capture_output=True,
        )
        subprocess.run(
            ["git", "push", "origin", f"work/soundness-mutations-shard-{SHARD}"],
            cwd=REPO,
            check=False,
            capture_output=True,
            timeout=120,
        )
    except (subprocess.TimeoutExpired, OSError):
        pass


def main() -> int:
    q7 = load_q7_gap_sites()
    OUT_ROOT.mkdir(parents=True, exist_ok=True)
    shard_dir = OUT_ROOT / f"shard-{SHARD}"
    shard_dir.mkdir(parents=True, exist_ok=True)
    tsv_path = shard_dir.with_suffix(".tsv")
    md_path = shard_dir.with_suffix(".md")
    progress_path = shard_dir / "progress.txt"

    fixtures = list_shard_fixtures()
    start = int(progress_path.read_text().strip() or "0") if progress_path.exists() else 0
    rows: list[Row] = []
    if os.environ.get("SOUNDNESS_FRESH") == "1":
        shutil.rmtree(shard_dir / "mutants", ignore_errors=True)
        progress_path.unlink(missing_ok=True)
        start = 0
    if start == 0:
        tsv_path.write_text(
            "fixture\ttier\tmutation_kind\tdetail\tbucket\tcurrent_verdict\troute_verdict\t"
            "route_detail\tdiscard_reason\tobligation\trule_file\trule_heading\t"
            "mutant_path\tcurrent_seconds\troute_seconds\n",
            encoding="utf-8",
        )

    originals_skipped = 0
    fixtures_zero_mutations = 0
    mutants_attempted = 0
    for idx in range(start, len(fixtures)):
        fixture = fixtures[idx]
        rel = fixture.relative_to(REPO).as_posix()
        tier = "pass" if "/pass/" in rel else "run"
        cur0, _, _, _ = run_omega(fixture)
        if not accepts(cur0):
            originals_skipped += 1
            progress_path.write_text(str(idx + 1), encoding="utf-8")
            continue
        original = read_source(fixture)
        mutations = generate_mutations(original)
        if not mutations:
            fixtures_zero_mutations += 1
        for m in mutations:
            mutants_attempted += 1
            mid = mutant_id(fixture, m.kind, m.detail)
            try:
                mutant_main = stage_mutant(fixture, m.text, mid)
            except OSError:
                row = Row(
                    fixture=rel,
                    tier=tier,
                    kind=m.kind,
                    detail=m.detail,
                    bucket="mutant_run_error",
                    current="error",
                    route="error",
                    route_detail="",
                    discard_reason="staging copy failed",
                    obligation="",
                    rule_file="",
                    rule_heading="",
                    rule_quote="",
                    mutant_path="",
                    current_s=0,
                    route_s=0,
                )
                rows.append(row)
                continue
            current_v, current_s, _, _ = run_omega(mutant_main)
            route_v, route_s, route_out = run_route(mutant_main)
            _, route_detail = parse_route_output(route_out)
            bucket, obligation, rfile, rhead, rquote, discard_reason = classify(
                m.kind,
                original,
                m.text,
                m,
                rel,
                current_v,
                route_v,
                route_detail,
                q7,
            )
            keep = bucket in ("confirmed_hole", "known_gap_q7_mut")
            rel_mutant = mutant_main.relative_to(REPO).as_posix()
            if not keep:
                shutil.rmtree(mutant_main.parent, ignore_errors=True)
                rel_mutant = mid
            row = Row(
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
                mutant_path=rel_mutant if keep else "",
                current_s=current_s,
                route_s=route_s,
            )
            rows.append(row)
            with tsv_path.open("a", encoding="utf-8") as f:
                f.write(
                    f"{rel}\t{tier}\t{m.kind}\t{m.detail}\t{bucket}\t{current_v}\t{route_v}\t"
                    f"{row.route_detail}\t{row.discard_reason}\t{row.obligation}\t{rfile}\t{rhead}\t"
                    f"{rel_mutant}\t{current_s:.3f}\t{route_s:.3f}\n"
                )
        progress_path.write_text(str(idx + 1), encoding="utf-8")
        if (idx + 1) % 25 == 0:
            write_md(
                md_path,
                len(fixtures),
                originals_skipped,
                fixtures_zero_mutations,
                mutants_attempted,
                rows,
                finished=False,
            )
            maybe_git_push()

    write_md(
        md_path,
        len(fixtures),
        originals_skipped,
        fixtures_zero_mutations,
        mutants_attempted,
        rows,
        finished=True,
    )
    maybe_git_push()
    return 0


if __name__ == "__main__":
    sys.exit(main())
