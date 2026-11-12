#!/usr/bin/env python3
"""Job 2 soundness mutation shard runner (facts only; no compiler edits)."""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Iterable

PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
REPO = Path(__file__).resolve().parents[3]
OMEGA = REPO / "target" / "debug" / "omega"
ROUTE = REPO / "target" / "debug" / "examples" / "route_one_comparison"

SPEC_QUOTES: dict[str, tuple[str, str]] = {
    "drop_requires": (
        "wiki/spec/language/state_contracts.md",
        "Machine `requires` applies at every invocation, including terminal and recursive "
        "calls. Every arrival establishes the target's requirements and declared domains "
        "under actual argument substitution.",
    ),
    "widen_requires": (
        "wiki/spec/language/state_contracts.md",
        "Machine `requires` applies at every invocation, including terminal and recursive "
        "calls. Every arrival establishes the target's requirements and declared domains "
        "under actual argument substitution.",
    ),
    "drop_divisor_guard": (
        "wiki/spec/language/expressions.md",
        "proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an",
    ),
    "drop_index_guard": (
        "wiki/spec/language/expressions.md",
        "machine [] read<Element [copy]>(items: &[Element], index: u64) -> Element\n"
        "requires index < items.len;",
    ),
    "remove_mut": (
        "wiki/spec/language/ownership.md",
        "modifying an established ordinary local's owned storage requires a `mut` binding. "
        "This covers direct assignment, writes to owned fields or array elements, and "
        "formation of `&mut` or `&write` loans to those places.",
    ),
    "flip_borrow_mutability": (
        "wiki/spec/language/ownership.md",
        "| `&mut T` | Exclusive observation and mutation. |",
    ),
    "write_through_shared_borrow": (
        "wiki/spec/language/ownership.md",
        "Reader and writer lists restrict observation and mutation of the exact "
        "qualified subject, including a qualified field reached through an owned parent.",
    ),
    "break_ensures": (
        "wiki/spec/language/state_contracts.md",
        "A written `ensures` is a minimum guarantee, not an instruction to forget stronger "
        "local facts.",
    ),
}


@dataclass
class Mutation:
    kind: str
    content: str


def slug(text: str) -> str:
    h = hashlib.sha256(text.encode()).hexdigest()[:10]
    return re.sub(r"[^a-z0-9_]+", "_", text.lower())[:40] + "_" + h


def mut_drop_first_requires(text: str) -> str | None:
    m = re.search(r"^\s*requires\s", text, flags=re.MULTILINE)
    if not m:
        return None
    start = m.start()
    depth = 0
    i = m.end()
    while i < len(text):
        c = text[i]
        if c == "(":
            depth += 1
        elif c == ")":
            if depth == 0:
                return text[:start] + text[i + 1 :]
            depth -= 1
        i += 1
    return None


def mut_widen_requires_nonzero(text: str) -> str | None:
    if "!=" not in text and "!= " not in text:
        return None
    new = re.sub(r"!=\s*0(\w*)", r"== 0\1", text, count=1)
    return new if new != text else None


def mut_drop_match_guard_division(text: str) -> str | None:
    pattern = re.compile(
        r"match\s+(\w+)\s*!=\s*0\w*\s*\{\s*true\s*->[^,]+,\s*_",
        re.DOTALL,
    )
    m = pattern.search(text)
    if not m:
        return None
    var = m.group(1)
    return pattern.sub(f"match false {{\n        true -> 10u32 / {var},\n        _", text, count=1)


def mut_remove_let_mut(text: str) -> str | None:
    new = re.sub(r"\blet\s+mut\s+", "let ", text, count=1)
    return new if new != text else None


def mut_amp_to_ampmut(text: str) -> str | None:
    if "&mut" in text:
        return None
    new = re.sub(r"&(?=write\s|[A-Za-z_])", "&mut ", text, count=1)
    return new if new != text else None


def mut_ampmut_to_amp(text: str) -> str | None:
    new = re.sub(r"&mut\s+", "&", text, count=1)
    return new if new != text else None


def mut_force_write_before_borrow_end(text: str) -> str | None:
    m = re.search(r"let\s+(\w+)\s*=\s*&([^;\n]+);", text)
    if not m:
        return None
    var, _ = m.group(1), m.group(2)
    ins = f"\n    {var}[0] = 0u8;\n"
    pos = m.end()
    return text[:pos] + ins + text[pos:]


def mut_break_ensures_zero(text: str) -> str | None:
    if "ensures" not in text:
        return None
    return re.sub(
        r"(ensures\s+[^(]+\([^)]*\)\s*>=\s*)(\d+\w*)",
        r"\g<1>999999u32",
        text,
        count=1,
    )


def gather_mutations(source: str) -> list[Mutation]:
    raw: list[tuple[str, Callable[[str], str | None]]] = [
        ("drop_requires", mut_drop_first_requires),
        ("widen_requires", mut_widen_requires_nonzero),
        ("drop_divisor_guard", mut_drop_match_guard_division),
        ("remove_mut", mut_remove_let_mut),
        ("flip_borrow_mutability", mut_amp_to_ampmut),
        ("flip_borrow_mutability", mut_ampmut_to_amp),
        ("write_through_shared_borrow", mut_force_write_before_borrow_end),
        ("break_ensures", mut_break_ensures_zero),
    ]
    out: list[Mutation] = []
    for kind, fn in raw:
        mutated = fn(source)
        if mutated is not None and mutated != source:
            out.append(Mutation(kind=kind, content=mutated))
    return out


def stage_mutant(main: Path, mutant_dir: Path, content: str) -> Path:
    if mutant_dir.exists():
        shutil.rmtree(mutant_dir)
    shutil.copytree(main.parent, mutant_dir)
    out = mutant_dir / "main.omg"
    out.write_text(content, encoding="utf-8")
    return out


def run_both_checkers(main_omg: Path, timeout_s: int) -> tuple[str, str]:
    """Run route_one_comparison once; it reports current= and route= for the harness."""
    env = os.environ.copy()
    env.setdefault("OMEGA_LIBRARY_ROOT", str(REPO / "source/library"))
    env.setdefault("OMEGA_CORPUS_ROOT", str(REPO))
    try:
        p = subprocess.run(
            [str(ROUTE), str(main_omg)],
            capture_output=True,
            text=True,
            timeout=timeout_s,
            env=env,
            stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        return "timeout", "timeout"
    current = "error"
    route = "error"
    for line in p.stdout.splitlines():
        if line.startswith("program "):
            m_cur = re.search(r"\bcurrent=([a-z_]+)", line)
            m_route = re.search(r"\broute=([a-z_]+)", line)
            if m_cur:
                current = m_cur.group(1)
            if m_route:
                route = m_route.group(1)
            if m_cur or m_route:
                break
    if "panicked" in p.stdout:
        return current, "defect"
    return current, route


def shard_fixtures(shard: int, mod_n: int = 8) -> list[Path]:
    proc = subprocess.run(
        [
            "git",
            "-C",
            str(REPO),
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    paths = sorted(proc.stdout.splitlines(), key=lambda s: s)
    return [REPO / paths[i] for i in range(len(paths)) if i % mod_n == shard]


def load_progress(progress_path: Path) -> set[str]:
    if not progress_path.exists():
        return set()
    return {line.strip() for line in progress_path.read_text().splitlines() if line.strip()}


@dataclass
class Counters:
    fixtures: int = 0
    originals_skipped: int = 0
    mutants_generated: int = 0
    mutants_run: int = 0
    kept_holes: int = 0
    kept_disagree: int = 0
    discarded_valid: int = 0
    discarded_no_rule: int = 0


def append_tsv(path: Path, row: Iterable[str]) -> None:
    with path.open("a", encoding="utf-8") as f:
        f.write("\t".join(row) + "\n")


def _md_lines(shard: int, counters: Counters, holes_md: list[str]) -> list[str]:
    return [
        "# Soundness mutations shard " + str(shard),
        "",
        f"PIN: `{PIN}`",
        "",
        "## Commands",
        "",
        "```bash",
        f"git checkout {PIN}",
        "export CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target",
        "export OMEGA_LIBRARY_ROOT=$PWD/source/library OMEGA_CORPUS_ROOT=$PWD",
        "timeout 120 nice -n 19 target/debug/examples/route_one_comparison <mutant>/main.omg < /dev/null",
        "mbx build -p omega -p compiler --example route_one_comparison",
        f"python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py --shard {shard} --resume",
        "```",
        "",
        "## Counts",
        "",
        f"- Fixtures visited this run: {counters.fixtures}",
        f"- Originals skipped (not accepted): {counters.originals_skipped}",
        f"- Mutants generated: {counters.mutants_generated}",
        f"- Mutants run: {counters.mutants_run}",
        f"- Kept holes (confirmed-invalid): {counters.kept_holes}",
        f"- Disagreements among kept: {counters.kept_disagree}",
        f"- Discarded valid: {counters.discarded_valid}",
        f"- Discarded no rule quote: {counters.discarded_no_rule}",
        "",
        "## Confirmed holes and disagreements",
        "",
        *holes_md,
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, default=0)
    ap.add_argument("--timeout", type=int, default=120)
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--limit", type=int, default=0, help="process at most N new fixtures (0=all)")
    args = ap.parse_args()

    out_dir = REPO / ".agents/four-jobs-2026-11-12/soundness"
    out_dir.mkdir(parents=True, exist_ok=True)
    tsv = out_dir / f"shard-{args.shard}.tsv"
    md = out_dir / f"shard-{args.shard}.md"
    progress = out_dir / f"shard-{args.shard}.progress"
    mut_root = out_dir / f"shard-{args.shard}" / "mutants"
    mut_root.mkdir(parents=True, exist_ok=True)

    if not args.resume or not tsv.exists():
        tsv.write_text(
            "fixture\tmutation_kind\tmutant_path\tcurrent_verdict\troute_verdict\t"
            "hand_verdict\tspec_path\tspec_quote\n",
            encoding="utf-8",
        )

    done_fixtures = load_progress(progress) if args.resume else set()
    counters = Counters()
    holes_md: list[str] = []

    fixtures = shard_fixtures(args.shard)
    processed_new = 0
    for main in fixtures:
        if args.limit and processed_new >= args.limit:
            break
        rel = str(main.relative_to(REPO))
        if rel in done_fixtures:
            continue
        counters.fixtures += 1
        processed_new += 1
        cur0, _ = run_both_checkers(main, args.timeout)
        if cur0 != "accepted":
            counters.originals_skipped += 1
            append_tsv(
                tsv,
                [
                    rel,
                    "_original_not_accepted",
                    "",
                    cur0,
                    "",
                    "discarded-valid",
                    "",
                    "",
                ],
            )
            continue

        source = main.read_text(encoding="utf-8")
        mutations = gather_mutations(source)
        counters.mutants_generated += len(mutations)
        fixture_had_row = False

        for mut in mutations:
            counters.mutants_run += 1
            mdir = mut_root / slug(rel) / mut.kind
            stage_mutant(main, mdir, mut.content)
            mrel = str((mdir / "main.omg").relative_to(REPO))

            current, route = run_both_checkers(mdir / "main.omg", args.timeout)

            accept_hole = current == "accepted" or route == "accepted"
            disagree = (
                current != route
                and "timeout" not in (current, route)
                and current not in ("", "error")
                and route not in ("", "error")
            )
            keep = accept_hole or disagree

            hand = "discarded-valid"
            spec_path = ""
            spec_quote = ""
            if keep:
                if mut.kind not in SPEC_QUOTES:
                    hand = "discarded-no-rule"
                    counters.discarded_no_rule += 1
                    shutil.rmtree(mdir, ignore_errors=True)
                else:
                    spec_path, spec_quote = SPEC_QUOTES[mut.kind]
                    hand = "confirmed-invalid"
                    counters.kept_holes += 1
                    if disagree:
                        counters.kept_disagree += 1
                    holes_md.append(
                        f"### {rel} — {mut.kind}\n\n"
                        f"- Original: `{rel}`\n"
                        f"- Mutant: `{mrel}`\n"
                        f"- Current checker: `{current}`; route: `{route}`\n"
                        f"- Hand check: **{hand}**\n"
                        f"- Spec ({spec_path}):\n\n"
                        f"> {spec_quote}\n"
                    )
            else:
                counters.discarded_valid += 1
                shutil.rmtree(mdir, ignore_errors=True)

            append_tsv(
                tsv,
                [
                    rel,
                    mut.kind,
                    mrel,
                    current,
                    route,
                    hand,
                    spec_path,
                    spec_quote.replace("\t", " ").replace("\n", " "),
                ],
            )
            fixture_had_row = True

        if not fixture_had_row:
            append_tsv(tsv, [rel, "_no_mutations", "", "", "", "discarded-valid", "", ""])

        with progress.open("a", encoding="utf-8") as pf:
            pf.write(rel + "\n")

        if counters.fixtures % 25 == 0:
            md.write_text(
                "\n".join(_md_lines(args.shard, counters, holes_md)),
                encoding="utf-8",
            )

    md.write_text("\n".join(_md_lines(args.shard, counters, holes_md)), encoding="utf-8")
    print(counters)
    return 0


if __name__ == "__main__":
    sys.exit(main())
