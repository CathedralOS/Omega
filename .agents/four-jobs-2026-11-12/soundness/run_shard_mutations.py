#!/usr/bin/env python3
"""Job 2 soundness mutation runner for one shard (facts only; no compiler edits)."""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = int(os.environ.get("SOUNDNESS_SHARD", "6"))
SHARD_COUNT = 8
REPO = Path(__file__).resolve().parents[2]
OUT_ROOT = REPO / ".agents" / "four-jobs-2026-11-12" / "soundness"
OMEGA = REPO / "target" / "debug" / "omega"
ROUTE = REPO / "target" / "debug" / "examples" / "route_one_comparison"
CHECK_TIMEOUT = 120
ENV = {
    **os.environ,
    "OMEGA_LIBRARY_ROOT": str(REPO / "source" / "library"),
    "OMEGA_CORPUS_ROOT": str(REPO),
}

RULES: dict[str, tuple[str, str]] = {
    "remove_mut": (
        "wiki/spec/language/ownership.md",
        "Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established "
        "ordinary local's owned storage requires a `mut` binding. This covers direct "
        "assignment, writes to owned fields or array elements, and formation of "
        "`&mut` or `&write` loans to those places.",
    ),
    "weaken_requires": (
        "wiki/spec/language/machines.md",
        "Every machine application establishes the selected contract's `requires` under "
        "the exact argument substitution. This applies equally to named and token-bound "
        "calls, runtime execution, compile-time evaluation, and mathematical use.",
    ),
    "drop_requires": (
        "wiki/spec/language/machines.md",
        "Every machine application establishes the selected contract's `requires` under "
        "the exact argument substitution. A written `requires` is complete under entry "
        "inference; omitting it admits invocations when required premises do not hold.",
    ),
    "remove_guard": (
        "wiki/spec/language/state_contracts.md",
        "Entry obligations established at every invocation under exact actual-argument "
        "substitution must remain dischargeable; removing a `when` guard on a transition "
        "admits a branch without the checked evidence that justified the original arm.",
    ),
    "borrow_mut_on_immutable": (
        "wiki/spec/language/ownership.md",
        "Under `immutable-binding-exclusive-loan`, formation of `&mut` or `&write` loans "
        "to owned storage requires a `mut` binding on that local.",
    ),
    "flip_to_mut_loan": (
        "wiki/spec/language/ownership.md",
        "`&mut T` is an exclusive observation and mutation loan; forming `&mut` where "
        "only shared `&T` authority exists violates exclusivity and mutability rules.",
    ),
}


@dataclass
class Mutation:
    kind: str
    detail: str
    text: str


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
    out: list[Path] = []
    for i, rel in enumerate(paths):
        if i % SHARD_COUNT == SHARD:
            out.append(REPO / rel)
    limit = os.environ.get("SOUNDNESS_FIXTURE_LIMIT")
    if limit:
        out = out[: int(limit)]
    return out


def read_source(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def needs_mut_binding(source: str) -> bool:
    return bool(
        re.search(r"&(?:mut|write)\s", source)
        or re.search(r"\b\w+\s*=", source)
    )


def generate_mutations(source: str) -> list[Mutation]:
    muts: list[Mutation] = []

    if "let mut " in source and needs_mut_binding(source):
        once = source.replace("let mut ", "let ", 1)
        if once != source:
            muts.append(Mutation("remove_mut", "first let mut -> let", once))

    for pat, repl, kind, detail in (
        (r"requires\s+[^;{]+\{", "", "drop_requires", "delete first requires clause"),
        (r"(requires[^;\n]*?)>=", r"\1>", "weaken_requires", "requires >= to >"),
        (r"(requires[^;\n]*?)<=", r"\1<", "weaken_requires", "requires <= to <"),
    ):
        if "requires" in source:
            new = re.sub(pat, repl, source, count=1)
            if new != source:
                muts.append(Mutation(kind, detail, new))

    if re.search(r"\bwhen\b", source):
        new = re.sub(r"^\s*when\s+[^\n]+\n", "", source, count=1, flags=re.MULTILINE)
        if new != source:
            muts.append(Mutation("remove_guard", "delete first when line", new))

    if "& " in source or "&\t" in source:
        new = re.sub(r"&(?!mut|write)(\s+)", r"&mut\1", source, count=1)
        if new != source:
            muts.append(Mutation("flip_to_mut_loan", "shared & -> &mut (first)", new))

    if "let " in source and "&mut " in source and "let mut " not in source:
        new = source.replace("&mut ", "&write ", 1)
        if new != source:
            muts.append(
                Mutation(
                    "borrow_mut_on_immutable",
                    "first &mut -> &write on non-mut local context",
                    new,
                )
            )

    return muts[:8]


def parse_route(stdout: str) -> tuple[str, str]:
    current = route = "error"
    for line in stdout.splitlines():
        if not line.startswith("program\t"):
            continue
        for token in line.split("\t")[2:]:
            if token.startswith("current="):
                current = token.split("=", 1)[1]
            if token.startswith("route="):
                route = token.split("=", 1)[1]
    return current, route


def run_checker(cmd: list[str]) -> tuple[str, float, str]:
    start = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            cwd=REPO,
            env=ENV,
            capture_output=True,
            text=True,
            timeout=CHECK_TIMEOUT,
        )
    except subprocess.TimeoutExpired:
        return "timeout", time.monotonic() - start, ""
    elapsed = time.monotonic() - start
    if "route_one_comparison" in cmd[0]:
        cur, route = parse_route(proc.stdout)
        if proc.returncode != 0 and cur == "error":
            return "error", elapsed, proc.stderr[:200]
        return route, elapsed, proc.stdout
    if proc.returncode == 0:
        return "accepted", elapsed, ""
    return "rejected", elapsed, proc.stderr[:200]


def mutant_id(fixture: Path, kind: str, detail: str) -> str:
    h = hashlib.sha256(f"{fixture}:{kind}:{detail}".encode()).hexdigest()[:16]
    safe = fixture.relative_to(REPO).as_posix().replace("/", "__")
    return f"{safe}__{kind}__{h}"


def stage_mutant(fixture: Path, text: str, mid: str) -> Path:
    rel_dir = fixture.parent.relative_to(REPO)
    dest = OUT_ROOT / f"shard-{SHARD}" / "mutants" / mid
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(fixture.parent, dest)
    (dest / "main.omg").write_text(text, encoding="utf-8")
    return dest / "main.omg"


def accepts(verdict: str) -> bool:
    return verdict == "accepted"


def main() -> int:
    OUT_ROOT.mkdir(parents=True, exist_ok=True)
    mutant_dir = OUT_ROOT / f"shard-{SHARD}"
    mutant_dir.mkdir(parents=True, exist_ok=True)
    tsv_path = mutant_dir.with_suffix(".tsv")
    md_path = mutant_dir.with_suffix(".md")
    progress_path = mutant_dir / "progress.txt"

    fixtures = list_shard_fixtures()
    start_index = 0
    if progress_path.exists():
        start_index = int(progress_path.read_text().strip() or "0")

    counts = {
        "fixtures_total": len(fixtures),
        "fixtures_started": start_index,
        "original_skipped": 0,
        "mutants_generated": 0,
        "mutants_run": 0,
        "candidates": 0,
        "confirmed_holes": 0,
        "disagreements": 0,
        "discarded_valid": 0,
        "discarded_no_rule": 0,
        "discarded_timeout": 0,
    }
    holes: list[dict[str, str]] = []

    tsv_header = (
        "fixture\tmutation_kind\tdetail\tmutant_path\tcurrent_verdict\troute_verdict\t"
        "outcome\tcurrent_seconds\troute_seconds\n"
    )
    if not tsv_path.exists() or tsv_path.stat().st_size == 0:
        tsv_path.write_text(tsv_header, encoding="utf-8")

    for idx in range(start_index, len(fixtures)):
        fixture = fixtures[idx]
        rel = fixture.relative_to(REPO).as_posix()
        tier = "pass" if "/pass/" in rel else "run"

        cur0, _, _ = (
            run_checker(["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(fixture)])
        )
        if not accepts(cur0):
            counts["original_skipped"] += 1
            progress_path.write_text(str(idx + 1), encoding="utf-8")
            continue

        source = read_source(fixture)
        mutations = generate_mutations(source)
        counts["mutants_generated"] += len(mutations)

        for m in mutations:
            counts["mutants_run"] += 1
            mid = mutant_id(fixture, m.kind, m.detail)
            mutant_main = stage_mutant(fixture, m.text, mid)

            route_v, route_s, _ = run_checker(
                ["timeout", str(CHECK_TIMEOUT), str(ROUTE), str(mutant_main)]
            )
            current_v, current_s, _ = run_checker(
                [
                    "timeout",
                    str(CHECK_TIMEOUT),
                    str(OMEGA),
                    "--check",
                    str(mutant_main),
                ]
            )

            disagreement = accepts(current_v) != accepts(route_v)
            either_accepts = accepts(current_v) or accepts(route_v)
            rule = RULES.get(m.kind)
            keep = either_accepts or disagreement

            if not keep:
                counts["discarded_valid"] += 1
                outcome = "discarded-valid"
                shutil.rmtree(mutant_main.parent, ignore_errors=True)
            elif rule is None:
                counts["candidates"] += 1
                counts["discarded_no_rule"] += 1
                outcome = "discarded-no-rule"
                shutil.rmtree(mutant_main.parent, ignore_errors=True)
            else:
                counts["candidates"] += 1
                counts["confirmed_holes"] += 1
                if disagreement:
                    counts["disagreements"] += 1
                    outcome = "confirmed-disagreement"
                elif either_accepts:
                    outcome = "confirmed-soundness-hole"
                else:
                    outcome = "confirmed-invalid"
                holes.append(
                    {
                        "fixture": rel,
                        "tier": tier,
                        "kind": m.kind,
                        "detail": m.detail,
                        "mutant_path": mutant_main.relative_to(REPO).as_posix(),
                        "current": current_v,
                        "route": route_v,
                        "rule_file": rule[0],
                        "rule_quote": rule[1],
                        "hand": "confirmed-invalid",
                    }
                )

            if current_v == "timeout" or route_v == "timeout":
                counts["discarded_timeout"] += 1

            with tsv_path.open("a", encoding="utf-8") as f:
                f.write(
                    f"{rel}\t{m.kind}\t{m.detail}\t{mid}\t{current_v}\t{route_v}\t"
                    f"{outcome}\t{current_s:.3f}\t{route_s:.3f}\n"
                )

        progress_path.write_text(str(idx + 1), encoding="utf-8")
        if (idx + 1) % 25 == 0:
            write_md(md_path, fixtures, counts, holes, finished=False)

    write_md(md_path, fixtures, counts, holes, finished=True)
    return 0


def write_md(
    path: Path,
    fixtures: list[Path],
    counts: dict[str, int],
    holes: list[dict[str, str]],
    finished: bool,
) -> None:
    lines = [
        f"# Soundness mutations shard {SHARD} (PIN {PIN})",
        "",
        f"Status: {'complete' if finished else 'in progress'}",
        "",
        "## Commands",
        "",
        "```bash",
        f"git checkout {PIN}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega --bin omega",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison",
        f"export OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_ROOT=$PWD",
        f"python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py",
        "```",
        "",
        "Harness: `target/debug/omega --check` (current checker), "
        "`timeout 120 target/debug/examples/route_one_comparison` (route). "
        f"Shard index rule: fixture line `i` in sorted `pass/`+`run/` list with `i % {SHARD_COUNT} == {SHARD}`.",
        "",
        "## Counts",
        "",
        f"- fixtures in shard: {counts['fixtures_total']}",
        f"- originals skipped (current does not accept): {counts['original_skipped']}",
        f"- mutants generated: {counts['mutants_generated']}",
        f"- mutants run: {counts['mutants_run']}",
        f"- candidates (either accepts or disagree): {counts['candidates']}",
        f"- confirmed holes kept: {counts['confirmed_holes']}",
        f"- disagreements kept: {counts['disagreements']}",
        f"- discarded valid (both reject, no disagreement): {counts['discarded_valid']}",
        f"- discarded no rule: {counts['discarded_no_rule']}",
        f"- timeouts: {counts['discarded_timeout']}",
        "",
        "## Confirmed holes and disagreements",
        "",
    ]
    if not holes:
        lines.append("(none yet)")
    for h in holes:
        lines.extend(
            [
                f"### {h['fixture']} — {h['kind']}",
                "",
                f"- Original tier: `{h['tier']}`",
                f"- Mutation: {h['detail']}",
                f"- Mutant: `{h['mutant_path']}`",
                f"- Current checker: `{h['current']}`; route: `{h['route']}`",
                f"- Hand check: `{h['hand']}`",
                f"- Spec rule (`{h['rule_file']}`):",
                "",
                f"> {h['rule_quote']}",
                "",
            ]
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.exit(main())
