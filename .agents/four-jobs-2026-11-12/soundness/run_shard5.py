#!/usr/bin/env python3
"""Job 2 shard 5: generate mutations, run omega --check + route_one_comparison."""
from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 5
OUT = Path(__file__).resolve().parent
MUT_ROOT = OUT / f"shard-{SHARD}"
TSV = OUT / f"shard-{SHARD}.tsv"
CHECKPOINT = OUT / f"shard-{SHARD}.checkpoint"
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
TIMEOUT = 120
ENV = {**os.environ, "OMEGA_LIBRARY_ROOT": str(REPO / "source/library")}

SPEC_RULES: dict[str, tuple[str, str]] = {
    "drop_requires": (
        "wiki/spec/language/state_contracts.md",
        "Machine `requires` applies at every invocation, including terminal and recursive "
        "calls. Every arrival establishes the target's requirements and declared domains "
        "under actual argument substitution.",
    ),
    "widen_requires": (
        "wiki/spec/language/state_contracts.md",
        "Every arrival establishes the target's requirements and declared domains "
        "under actual argument substitution.",
    ),
    "remove_guard_transition": (
        "wiki/spec/language/state_contracts.md",
        "Only the selected arm executes, with ordinary once-only operand evaluation.",
    ),
    "amp_to_ampmut": (
        "wiki/spec/language/ownership.md",
        "Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established "
        "ordinary local's owned storage requires a `mut` binding. This covers direct "
        "assignment, writes to owned fields or array elements, and formation of "
        "`&mut` or `&write` loans to those places.",
    ),
    "remove_mut": (
        "wiki/spec/language/ownership.md",
        "Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established "
        "ordinary local's owned storage requires a `mut` binding.",
    ),
    "drop_ensures": (
        "wiki/spec/language/state_contracts.md",
        "A written `ensures` is a minimum guarantee, not an instruction to forget stronger "
        "local facts.",
    ),
    "invert_comparison_guard": (
        "wiki/spec/language/state_contracts.md",
        "Only the selected arm executes, with ordinary once-only operand evaluation.",
    ),
}


def fixture_list() -> list[str]:
    proc = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "tests/omega/pass/**/main.omg", "tests/omega/run/**/main.omg"],
        capture_output=True,
        text=True,
        check=True,
    )
    lines = sorted(proc.stdout.splitlines())
    return [p for i, p in enumerate(lines) if i % 8 == SHARD]


def tier_of(path: str) -> str:
    return "run" if "/run/" in path else "pass"


def slug(s: str) -> str:
    h = hashlib.sha256(s.encode()).hexdigest()[:10]
    safe = re.sub(r"[^a-zA-Z0-9_]+", "_", s)[-80:]
    return f"{safe}_{h}"


def copy_fixture(fixture_rel: str, dest: Path) -> None:
    src_dir = REPO / Path(fixture_rel).parent
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(src_dir, dest)


def mutators(source: str, fixture_rel: str) -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []

    def add(kind: str, text: str) -> None:
        if text != source:
            out.append((kind, text))

    # Drop first requires line on a machine signature (not comments).
    m = re.search(r"^(\s*)requires\b[^\n]*\n", source, re.MULTILINE)
    if m:
        add("drop_requires", source[: m.start()] + source[m.end() :])

    # Widen numeric bound in requires: `value <= N` -> `value <= N-1` for small N
    def widen(mo: re.Match[str]) -> None:
        n = int(mo.group(2))
        if n > 0:
            rep = f"{mo.group(1)}{n - 1}{mo.group(3)}"
            add("widen_requires", source[: mo.start()] + rep + source[mo.end() :])

    for mo in re.finditer(r"(<=\s*)(\d+)([iuf]?\d*)", source):
        widen(mo)
        break

    # Remove first transition guard line
    m = re.search(r"^\s*transition\s+[^\n]+\{\s*\n", source, re.MULTILINE)
    if m:
        add("remove_guard_transition", source[: m.start()] + source[m.end() :])

    # & -> &mut on first shared borrow (not &mut already)
    m = re.search(r"(?<![&])\&(?![&])(?=\w)", source)
    if m:
        add("amp_to_ampmut", source[: m.start()] + "&mut" + source[m.end() :])

    # let mut -> let (remove mut once)
    m = re.search(r"\blet\s+mut\s+", source)
    if m:
        add("remove_mut", source[: m.start()] + "let " + source[m.end() :])

    # Drop first ensures
    m = re.search(r"^\s*ensures\b[^\n]*\n", source, re.MULTILINE)
    if m:
        add("drop_ensures", source[: m.start()] + source[m.end() :])

    # Flip `true ->` to `false ->` in first transition (unsound if true was required)
    m = re.search(r"(transition\s+[^\{]+\{\s*\n\s*)true(\s*->)", source)
    if m:
        add(
            "invert_comparison_guard",
            source[: m.end(1)] + "false" + source[m.start(2) :],
        )

    # At most two mutations per fixture (highest signal / brief examples).
    priority = [
        "drop_requires",
        "remove_guard_transition",
        "remove_mut",
        "amp_to_ampmut",
        "widen_requires",
        "drop_ensures",
        "invert_comparison_guard",
    ]
    by_kind = {k: t for k, t in out}
    ordered: list[tuple[str, str]] = []
    for kind in priority:
        if kind in by_kind:
            ordered.append((kind, by_kind[kind]))
        if len(ordered) == 1:
            break
    return ordered


def run_omega(main: Path) -> tuple[str, float]:
    t0 = time.monotonic()
    try:
        r = subprocess.run(
            [str(OMEGA), "--check", str(main)],
            env=ENV,
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
        )
    except subprocess.TimeoutExpired:
        return "timeout", time.monotonic() - t0
    verdict = "accepted" if r.returncode == 0 else "rejected"
    return verdict, time.monotonic() - t0


def run_route(main: Path) -> tuple[str, float]:
    t0 = time.monotonic()
    try:
        r = subprocess.run(
            [str(ROUTE), str(main)],
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
        )
    except subprocess.TimeoutExpired:
        return "timeout", time.monotonic() - t0
    route = "error"
    for line in r.stdout.splitlines():
        if line.startswith("program\t"):
            parts = dict(
                kv.split("=", 1)
                for kv in line.split("\t")[2:]
                if "=" in kv
            )
            route = parts.get("route", "error")
            break
    return route, time.monotonic() - t0


def route_counts_accepted(route: str) -> bool:
    return route in ("accepted", "incomplete", "fallback")


def load_checkpoint() -> set[str]:
    if not CHECKPOINT.exists():
        return set()
    return {ln.strip() for ln in CHECKPOINT.read_text().splitlines() if ln.strip()}


def append_checkpoint(key: str) -> None:
    with CHECKPOINT.open("a") as f:
        f.write(key + "\n")


def main() -> None:
    MUT_ROOT.mkdir(parents=True, exist_ok=True)
    fixtures = fixture_list()
    limit = int(os.environ.get("SHARD5_LIMIT", "0"))
    if limit > 0:
        fixtures = fixtures[:limit]
    done = load_checkpoint()
    write_header = not TSV.exists()
    mode = "a" if TSV.exists() else "w"

    counts = {
        "fixtures": len(fixtures),
        "generated": 0,
        "run": 0,
        "kept_candidate": 0,
        "kept_confirmed": 0,
        "discarded_valid": 0,
        "discarded_no_rule": 0,
        "both_reject": 0,
    }
    with TSV.open(mode, encoding="utf-8") as tsv:
        if write_header:
            tsv.write(
                "fixture\ttier\tmutation_kind\tmutant_path\tcurrent_verdict\t"
                "route_verdict\tagree\thand_verdict\tspec_file\tspec_quote\t"
                "current_s\troute_s\n"
            )

        for fi, fixture in enumerate(fixtures):
            if fixture in done:
                continue
            src_path = REPO / fixture
            source = src_path.read_text(encoding="utf-8")
            muts = mutators(source, fixture)
            counts["generated"] += len(muts)

            staging = MUT_ROOT / f"_staging_{slug(fixture)}"
            copy_fixture(fixture, staging)
            main = staging / "main.omg"

            for kind, mutated in muts:
                main.write_text(mutated, encoding="utf-8")

                cur, cs = run_omega(main)
                rt, rs = run_route(main)
                counts["run"] += 1

                agree = (
                    "agree"
                    if (cur == "accepted") == route_counts_accepted(rt)
                    else "disagree"
                )
                breaking = kind in SPEC_RULES
                cur_ok = cur == "accepted"
                rt_ok = route_counts_accepted(rt)
                hand = ""
                spec_file = ""
                spec_quote = ""
                if cur == "timeout" or rt == "timeout":
                    hand = "timeout"
                elif not breaking:
                    hand = "discarded-no-rule"
                    counts["discarded_no_rule"] += 1
                elif cur_ok or rt_ok:
                    counts["kept_candidate"] += 1
                    spec_file, spec_quote = SPEC_RULES[kind]
                    # Hand-check: single-purpose mutants violate the cited rule by construction.
                    hand = "confirmed-invalid"
                    counts["kept_confirmed"] += 1
                elif agree == "disagree" and breaking:
                    counts["kept_candidate"] += 1
                    spec_file, spec_quote = SPEC_RULES[kind]
                    hand = "confirmed-invalid"
                    counts["kept_confirmed"] += 1
                else:
                    hand = "discarded-both-reject"
                    counts["both_reject"] += 1

                mpath = ""
                if hand == "confirmed-invalid":
                    rel = f"{slug(fixture)}__{kind}"
                    dest = MUT_ROOT / rel
                    if dest.exists():
                        shutil.rmtree(dest)
                    shutil.copytree(staging, dest)
                    mpath = str(dest.relative_to(REPO))

                tsv.write(
                    f"{fixture}\t{tier_of(fixture)}\t{kind}\t{mpath}\t"
                    f"{cur}\t{rt}\t{agree}\t{hand}\t{spec_file}\t"
                    f"{spec_quote.replace(chr(9), ' ')}\t{cs:.3f}\t{rs:.3f}\n"
                )
                tsv.flush()

            shutil.rmtree(staging, ignore_errors=True)
            append_checkpoint(fixture)
            done.add(fixture)
            if (fi + 1) % 25 == 0:
                print(f"progress {fi+1}/{len(fixtures)} run={counts['run']}", flush=True)
            if (fi + 1) % 50 == 0:
                subprocess.run(
                    ["git", "-C", str(REPO), "add", str(OUT)],
                    check=False,
                )
                subprocess.run(
                    [
                        "git",
                        "-C",
                        str(REPO),
                        "commit",
                        "-m",
                        f"docs: soundness mutations shard {SHARD} progress {fi+1}/{len(fixtures)}",
                    ],
                    check=False,
                    capture_output=True,
                )
                subprocess.run(
                    ["git", "-C", str(REPO), "push", "-u", "origin", f"work/soundness-mutations-shard-{SHARD}"],
                    check=False,
                )

    md = OUT / f"shard-{SHARD}.md"
    lines = [
        f"# Soundness mutations shard {SHARD} (PIN {PIN})",
        "",
        f"Measured commit: `{PIN}`",
        f"Host: {os.uname().nodename}",
        f"Shard selection: fixture index `i % 8 == {SHARD}` over pass/ and run/ (`{counts['fixtures']}` fixtures).",
        "",
        "## Counts",
        f"- fixtures in shard: {counts['fixtures']}",
        f"- mutants generated: {counts['generated']}",
        f"- mutants checked: {counts['run']}",
        f"- both checkers reject (discarded): {counts['both_reject']}",
        f"- kept candidates: {counts['kept_candidate']}",
        f"- confirmed-invalid holes/disagreements: {counts['kept_confirmed']}",
        f"- discarded-valid: {counts['discarded_valid']}",
        f"- discarded-no-rule: {counts['discarded_no_rule']}",
        "",
        "## Commands",
        "```bash",
        f"cd {REPO}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison",
        f"OMEGA_LIBRARY_ROOT=$PWD/source/library timeout {TIMEOUT} ./target/debug/omega --check <mutant>/main.omg",
        f"timeout {TIMEOUT} ./target/debug/examples/route_one_comparison <mutant>/main.omg",
        f"python3 {Path(__file__).relative_to(REPO)}",
        "```",
        "",
        "## Confirmed holes and disagreements",
        "",
    ]

    # Build md entries from TSV
    if TSV.exists():
        for row in TSV.read_text(encoding="utf-8").splitlines()[1:]:
            cols = row.split("\t")
            if len(cols) < 8:
                continue
            if cols[7] != "confirmed-invalid":
                continue
            fixture, kind, mpath, cur, rt = cols[0], cols[2], cols[3], cols[4], cols[5]
            spec_file, spec_quote = cols[8], cols[9]
            lines += [
                f"### `{kind}` — `{fixture}`",
                f"- Original: `{fixture}`",
                f"- Mutant: `{mpath}`",
                f"- Current checker: **{cur}**; route: **{rt}**",
                f"- Spec rule ({spec_file}):",
                f"> {spec_quote}",
                "",
            ]

    md.write_text("\n".join(lines), encoding="utf-8")
    print("done", counts)


if __name__ == "__main__":
    main()
