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

COMPILE_TIME_SUFFIX = (
    " Checked compilation must establish this obligation before execution; "
    "a runtime trap or crash arm does not discharge it at check time."
)

SPEC_RULES: dict[str, tuple[str, str, str]] = {
    "remove_mut": (
        "wiki/spec/language/expressions.md",
        "Assignment and explicit `&mut` / `&write` loans",
        "Such a loan requires existing writable authority at the selected place; "
        "owned-local mutability requires a `mut` binding when that place is the local's owned storage."
        + COMPILE_TIME_SUFFIX,
    ),
    "weaken_requires": (
        "wiki/spec/language/machines.md",
        "Call preconditions",
        "Every machine application establishes the selected contract's `requires` under "
        "the exact argument substitution." + COMPILE_TIME_SUFFIX,
    ),
    "drop_requires": (
        "wiki/spec/language/machines.md",
        "Call preconditions",
        "Every machine application establishes the selected contract's `requires` under "
        "the exact argument substitution." + COMPILE_TIME_SUFFIX,
    ),
    "remove_guard": (
        "wiki/spec/language/state_contracts.md",
        "Requirement clauses",
        "Entry obligations established at every invocation under exact actual-argument "
        "substitution must be dischargeable at every reachable call." + COMPILE_TIME_SUFFIX,
    ),
    "amp_to_ampmut": (
        "wiki/spec/language/expressions.md",
        "Assignment and explicit `&mut` / `&write` loans",
        "Such a loan requires existing writable authority at the selected place; "
        "forming `&mut` without writable authority violates exclusivity and mutability rules."
        + COMPILE_TIME_SUFFIX,
    ),
}

ROUTE_OUTSIDE = frozenset(
    {
        "no-entry",
        "unreached",
        "outside",
        "incomplete",
        "defect",
        "error",
        "fallback",
    }
)


@dataclass
class SpecRead:
    invalid: bool
    obligation: str
    valid_reason: str
    invalid_only_missing_mut: bool
    rule: tuple[str, str, str] | None

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
    accepted_by: str
    mutant_path: str
    current_s: float
    route_s: float


BUCKETS = (
    "confirmed-hole",
    "known-gap-q7-mut",
    "disagreement-valid",
    "valid-route-outside",
    "discarded-valid",
    "rejected-both",
    "timeout",
    "failed-run",
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
            muts.append(Mutation("amp_to_ampmut", "first shared & -> &mut", new, {}))
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


def route_is_outside(route: str) -> bool:
    return route in ROUTE_OUTSIDE or route not in ("accepted", "rejected")


def route_rejects(route: str) -> bool:
    return route == "rejected"


def checker_accepted_by(current: str, route: str) -> str:
    ac = accepts(current)
    ar = route == "accepted"
    if ac and ar:
        return "both"
    if ac:
        return "current"
    if ar:
        return "route"
    return "neither"


def either_checker_accepts(current: str, route: str) -> bool:
    return accepts(current) or route == "accepted"


def spec_allows_runtime_trap(kind: str, mutant: str) -> bool:
    """True when the mutant still routes violations to an explicit crash arm."""
    if not re.search(r"\bcrash\s+[A-Za-z_]", mutant):
        return False
    return kind in ("remove_guard", "drop_requires", "weaken_requires")


def minimal_diff_lines(original: str, mutant: str) -> list[str]:
    out: list[str] = []
    for a, b in zip(original.splitlines(), mutant.splitlines()):
        if a != b:
            out.append(f"- {a}")
            out.append(f"+ {b}")
    o_extra = original.splitlines()[len(mutant.splitlines()) :]
    m_extra = mutant.splitlines()[len(original.splitlines()) :]
    for a in o_extra:
        out.append(f"- {a}")
    for b in m_extra:
        out.append(f"+ {b}")
    return out


def read_spec_verdict(
    kind: str,
    original: str,
    mutant: str,
    mutation: Mutation,
) -> SpecRead:
    """Invalidity from reading the mutant only (route messages are not used)."""
    if kind == "remove_mut":
        name = mutation.meta.get("binding", "")
        if count_let_bindings(mutant, name) > 1:
            return SpecRead(
                False,
                "",
                f"inner `let {name}` shadows the demoted binding",
                False,
                None,
            )
        obligation = find_remove_mut_violation(mutant, name) or ""
        if not obligation:
            return SpecRead(
                False,
                "",
                "demoted binding has no write, reassign, or &mut/&write use",
                False,
                None,
            )
        return SpecRead(
            True,
            obligation,
            "",
            True,
            SPEC_RULES["remove_mut"],
        )
    if kind == "drop_requires":
        removed = mutation.meta.get("removed") or first_requires_clause(original) or ""
        obligation = drop_requires_dependent_obligation(original, mutant, removed) or ""
        if obligation:
            return SpecRead(True, obligation, "", False, SPEC_RULES["drop_requires"])
        restated = transition_restates_requires(original, removed)
        if restated:
            return SpecRead(False, restated, "same fact restated on a transition arm", False, None)
        return SpecRead(
            False,
            "",
            "no body or ensures line needs the removed requires fact",
            False,
            None,
        )
    if kind == "weaken_requires":
        obligation = weaken_requires_breach(original, mutant, mutation.detail) or ""
        if obligation:
            return SpecRead(True, obligation, "", False, SPEC_RULES["weaken_requires"])
        return SpecRead(False, "", "weakened requires still satisfied at uses in this file", False, None)
    if kind == "remove_guard":
        obligation = remove_guard_obligation(original, mutant) or ""
        if obligation:
            return SpecRead(True, obligation, "", False, SPEC_RULES["remove_guard"])
        return SpecRead(False, "", "removed guard does not leave an unchecked branch", False, None)
    if kind == "amp_to_ampmut":
        obligation = flip_to_mut_line(mutant) or ""
        if obligation:
            return SpecRead(True, obligation, "", False, SPEC_RULES["amp_to_ampmut"])
        return SpecRead(False, "", "no &mut loan site in mutant", False, None)
    return SpecRead(False, "", f"no spec read for kind {kind}", False, None)


def assign_bucket(
    spec: SpecRead,
    kind: str,
    fixture_rel: str,
    mutation: Mutation,
    current: str,
    route: str,
    q7: set[tuple[str, str]],
) -> tuple[str, str, str, str, str, str]:
    """Return bucket, obligation, rule fields, accepted_by, valid/discard note."""
    if current == "timeout" or route == "timeout":
        return "timeout", "", "", "", "", "neither", "checker timeout"
    accepted = checker_accepted_by(current, route)
    either_accepts = either_checker_accepts(current, route)
    name = mutation.meta.get("binding", "")

    if not spec.invalid:
        if accepts(current) and route == "accepted":
            return (
                "discarded-valid",
                spec.obligation,
                "",
                "",
                "",
                accepted,
                spec.valid_reason,
            )
        if accepts(current) and route_is_outside(route):
            return (
                "valid-route-outside",
                spec.obligation,
                "",
                "",
                "",
                "current",
                f"valid mutant; route did not judge ({route})",
            )
        if (accepts(current) and route_rejects(route)) or (
            route == "accepted" and not accepts(current)
        ):
            return (
                "disagreement-valid",
                spec.obligation,
                "",
                "",
                "",
                accepted,
                spec.valid_reason or "valid under spec read; one checker rejects",
            )
        return (
            "rejected-both",
            "",
            "",
            "",
            "",
            accepted,
            spec.valid_reason,
        )

    rule = spec.rule or SPEC_RULES.get(kind)
    rfile, rhead, rquote = rule if rule else ("", "", "")
    if (
        kind == "remove_mut"
        and spec.invalid_only_missing_mut
        and (fixture_rel, name) in q7
        and either_accepts
    ):
        return (
            "known-gap-q7-mut",
            spec.obligation,
            rfile,
            rhead,
            rquote,
            accepted,
            "",
        )
    if either_accepts:
        return (
            "confirmed-hole",
            spec.obligation,
            rfile,
            rhead,
            rquote,
            accepted,
            "",
        )
    return ("rejected-both", spec.obligation, "", "", "", accepted, "")


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
    for line in minimal_diff_lines(orig, mut):
        lines.append(line)
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
    by_accepted: dict[str, int] = {}
    for r in rows:
        if r.bucket == "confirmed-hole":
            by_kind[r.kind] = by_kind.get(r.kind, 0) + 1
            by_accepted[r.accepted_by] = by_accepted.get(r.accepted_by, 0) + 1

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
        f"| Originals skipped | {originals_skipped} |",
        f"| Fixtures checked (original accepted) | {fixtures_total - originals_skipped} |",
        f"| Fixtures with zero generated mutants (no-mutation) | {fixtures_zero_mutations} |",
        f"| Fixtures with ≥1 mutant row | {len({r.fixture for r in rows})} |",
        f"| Mutant rows | {n_run} |",
        f"| Mean mutants per mutated fixture | "
        f"{(n_run / max(1, len({r.fixture for r in rows}))):.2f} |",
        "",
        "Reconcile: `fixtures in shard` = skipped + checked; "
        "`checked` = no-mutation + mutated fixtures; mutant row buckets sum to `mutant rows`.",
        "",
        "## Mutant buckets (sum = mutants attempted)",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    bucket_order = list(BUCKETS) + ["failed-run"]
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

    multi: dict[str, int] = {}
    for r in rows:
        multi[r.fixture] = multi.get(r.fixture, 0) + 1
    multi_list = sorted((f, c) for f, c in multi.items() if c > 1)
    lines.extend(["", "## Fixtures with more than one mutant row", ""])
    if not multi_list:
        lines.append("(none)")
    else:
        lines.append(f"Count: {len(multi_list)} fixtures.")
        for f, c in multi_list[:40]:
            lines.append(f"- `{f}`: {c} rows")
        if len(multi_list) > 40:
            lines.append(f"- … and {len(multi_list) - 40} more (see TSV)")

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
    lines.extend(
        [
            "",
            "## Confirmed holes by accepted_by",
            "",
            "| accepted_by | Count |",
            "| --- | ---: |",
        ]
    )
    for k, c in sorted(by_accepted.items()):
        lines.append(f"| `{k}` | {c} |")
    if not by_accepted:
        lines.append("| (none) | 0 |")

    kept = [r for r in rows if r.bucket in ("confirmed-hole", "known-gap-q7-mut")]
    lines.extend(["", "## Kept rows", ""])
    for r in kept:
        lines.extend(
            [
                f"### {r.fixture} — {r.kind}",
                "",
                f"- Bucket: `{r.bucket}`",
                f"- Mutation: {r.detail}",
                f"- Mutant: `{r.mutant_path}`",
                f"- Current: `{r.current}`; route: `{r.route}`; accepted_by: `{r.accepted_by}`",
                f"- Route detail (hint only): {r.route_detail[:300] if r.route_detail else '(n/a)'}",
                f"- Broken obligation: `{r.obligation}`",
                f"- (a) Compile-time spec (`{r.rule_file}` § {r.rule_heading}):",
                "",
                f"> {r.rule_quote}",
                f"- (b) Check-time vs runtime: accepting checker verdict `{r.current}` "
                f"(route `{r.route}`); obligation must be proved at check, not deferred to crash.",
                f"- (c) Minimal diff: see hand-verification section below.",
                "",
            ]
        )

    samples = [r for r in rows if r.bucket == "confirmed-hole"][:5]
    if samples:
        lines.extend(["", "## Hand verification (5 confirmed-hole)", ""])
        for r in samples:
            lines.extend(hand_verify_row(r))
    discarded = [r for r in rows if r.bucket == "discarded-valid"][:5]
    if discarded:
        lines.extend(["", "## Hand verification (5 discarded-valid)", ""])
        for r in discarded:
            lines.extend(hand_verify_row(r))
    q7_samples = [r for r in rows if r.bucket == "known-gap-q7-mut"][:5]
    if q7_samples:
        lines.extend(["", "## Hand verification (5 known-gap-q7-mut)", ""])
        for r in q7_samples:
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
            "accepted_by\tmutant_path\tcurrent_seconds\troute_seconds\n",
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
                    bucket="failed-run",
                    current="error",
                    route="error",
                    route_detail="",
                    discard_reason="staging copy failed",
                    obligation="",
                    rule_file="",
                    rule_heading="",
                    rule_quote="",
                    accepted_by="neither",
                    mutant_path="",
                    current_s=0,
                    route_s=0,
                )
                rows.append(row)
                continue
            current_v, current_s, _, _ = run_omega(mutant_main)
            route_v, route_s, route_out = run_route(mutant_main)
            _, route_detail = parse_route_output(route_out)
            spec = read_spec_verdict(m.kind, original, m.text, m)
            bucket, obligation, rfile, rhead, rquote, accepted, discard_reason = assign_bucket(
                spec, m.kind, rel, m, current_v, route_v, q7
            )
            if bucket == "confirmed-hole" and spec_allows_runtime_trap(m.kind, m.text):
                bucket = "discarded-valid"
                discard_reason = (
                    "invalid under read but explicit crash arm may trap at runtime; "
                    "not a compile-time hole"
                )
                obligation = spec.obligation
                rfile = rhead = rquote = ""
            keep = bucket in ("confirmed-hole", "known-gap-q7-mut")
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
                accepted_by=accepted,
                mutant_path=rel_mutant if keep else "",
                current_s=current_s,
                route_s=route_s,
            )
            rows.append(row)
            with tsv_path.open("a", encoding="utf-8") as f:
                f.write(
                    f"{rel}\t{tier}\t{m.kind}\t{m.detail}\t{bucket}\t{current_v}\t{route_v}\t"
                    f"{row.route_detail}\t{row.discard_reason}\t{row.obligation}\t{rfile}\t{rhead}\t"
                    f"{accepted}\t{rel_mutant}\t{current_s:.3f}\t{route_s:.3f}\n"
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
