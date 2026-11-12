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

sys.path.insert(0, str(Path(__file__).resolve().parent))
from soundness_validate import classify  # noqa: E402

PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
REPO = Path(__file__).resolve().parents[3]
ROUTE = REPO / "target" / "debug" / "examples" / "route_one_comparison"


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
        if line.startswith("program\t") or line.startswith("program "):
            m_cur = re.search(r"\bcurrent=([\w-]+)", line)
            m_route = re.search(r"\broute=([\w-]+)", line)
            if m_cur:
                current = m_cur.group(1)
            if m_route:
                route = m_route.group(1)
            if m_cur or m_route:
                break
    route_reasons: list[str] = []
    for line in p.stdout.splitlines():
        if line.startswith(
            ("diagnostic\t", "fallback\t", "use\t", "current-diagnostic\t", "obligation\t")
        ):
            route_reasons.append(line.replace("\t", " ")[:400])
    route_reason = "; ".join(route_reasons[:3])
    if not route_reason:
        route_reason = f"route_verdict={route}"
    if "panicked" in p.stdout:
        return current, "defect", route_reason
    return current, route, route_reason


def fixtures_in_tsv(tsv: Path) -> set[str]:
    if not tsv.exists():
        return set()
    out: set[str] = set()
    for line in tsv.read_text().splitlines()[1:]:
        parts = line.split("\t")
        if parts and not parts[1].startswith("_"):
            out.add(parts[0])
        elif parts and parts[1].startswith("_"):
            out.add(parts[0])
    return out


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


def _md_lines(
    shard: int, counters: Counters, holes_md: list[str], totals: dict[str, int]
) -> list[str]:
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
        f"- Fixtures completed (progress file): {_progress_count(shard)}",
        f"- Originals skipped (not accepted): {totals['originals_skipped']}",
        f"- Mutants generated (this run): {counters.mutants_generated}",
        f"- Mutants run (cumulative): {totals['mutants_run']}",
        "## Mutant bucket table (sums to mutant total)",
        "",
        f"| Bucket | Count |",
        f"| --- | ---: |",
        f"| confirmed-invalid | {totals['confirmed_invalid']} |",
        f"| known-gap-q7-mut | {totals['known_gap_q7']} |",
        f"| discarded-valid | {totals['discarded_valid']} |",
        f"| discarded-no-rule | {totals['discarded_no_rule']} |",
        f"| mutant-run-failed | {totals['mutant_run_failed']} |",
        f"| mutant-generate-failed | {totals['mutant_generate_failed']} |",
        f"| **mutant total** | **{totals['mutants_run']}** |",
        "",
        f"Baseline mutant total before this resume pass: {totals.get('baseline_mutants', 'see meta')}. "
        f"Delta explained in shard-{shard}.meta.json.",
        "",
        *_downgraded_line(shard, totals),
        "",
        f"- Confirmed-invalid by mutation kind: {totals.get('confirmed_by_kind', {})}",
        "",
        "## Confirmed-invalid and known-gap rows",
        "",
        *holes_md,
    ]


def _downgraded_line(shard: int, totals: dict) -> list[str]:
    import json

    meta = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{shard}.meta.json"
    down = totals.get("downgraded", 0)
    if meta.exists():
        down = json.loads(meta.read_text()).get("downgraded_from_confirmed", down)
    return [
        f"Downgraded from pre-review `confirmed-invalid`: {down} "
        f"(reclassified into the buckets above, chiefly `discarded-valid`).",
    ]


def _progress_count(shard: int) -> int:
    progress = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{shard}.progress"
    if not progress.exists():
        return 0
    return sum(1 for line in progress.read_text().splitlines() if line.strip())


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, default=0)
    ap.add_argument("--timeout", type=int, default=120)
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--limit", type=int, default=0, help="process at most N new fixtures (0=all)")
    ap.add_argument(
        "--fresh",
        action="store_true",
        help="truncate shard tsv (default: refuse if tsv exists without --fresh)",
    )
    args = ap.parse_args()

    out_dir = REPO / ".agents/four-jobs-2026-11-12/soundness"
    out_dir.mkdir(parents=True, exist_ok=True)
    tsv = out_dir / f"shard-{args.shard}.tsv"
    md = out_dir / f"shard-{args.shard}.md"
    progress = out_dir / f"shard-{args.shard}.progress"
    mut_root = out_dir / f"shard-{args.shard}" / "mutants"
    mut_root.mkdir(parents=True, exist_ok=True)

    header = (
        "fixture\tmutation_kind\tmutant_path\tcurrent_verdict\troute_verdict\t"
        "hand_verdict\tspec_path\tspec_quote\tobligation_line\troute_reason\treview_reason\n"
    )
    meta = out_dir / f"shard-{args.shard}.meta.json"
    if tsv.exists() and not args.resume and not args.fresh:
        print("refusing to truncate existing tsv; use --resume or --fresh", file=sys.stderr)
        return 2
    if not tsv.exists() or args.fresh:
        tsv.write_text(header, encoding="utf-8")

    done_fixtures = set()
    if args.resume:
        done_fixtures |= load_progress(progress)
        done_fixtures |= fixtures_in_tsv(tsv)
    import json

    baseline = _totals_from_tsv(tsv)["mutants_run"] if tsv.exists() else 0
    meta.write_text(
        json.dumps({"baseline_mutants": baseline, "pin": PIN}, indent=2) + "\n",
        encoding="utf-8",
    )
    counters = Counters()
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
        cur0, _, _ = run_both_checkers(main, args.timeout)
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
                    "",
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
            try:
                stage_mutant(main, mdir, mut.content)
            except OSError as exc:
                append_tsv(
                    tsv,
                    [
                        rel,
                        "_mutant_generate_failed",
                        mut.kind,
                        "",
                        "",
                        "mutant-generate-failed",
                        "",
                        "",
                        "",
                        "",
                        str(exc)[:200],
                    ],
                )
                fixture_had_row = True
                continue
            mrel = str((mdir / "main.omg").relative_to(REPO))

            try:
                current, route, route_reason = run_both_checkers(mdir / "main.omg", args.timeout)
            except Exception as exc:  # noqa: BLE001
                append_tsv(
                    tsv,
                    [
                        rel,
                        mut.kind,
                        mrel,
                        "error",
                        "error",
                        "mutant-run-failed",
                        "",
                        "",
                        "",
                        str(exc)[:200],
                        "",
                    ],
                )
                counters.mutants_run += 0
                fixture_had_row = True
                shutil.rmtree(mdir, ignore_errors=True)
                continue

            hand, spec_path, spec_quote, obligation, review_reason = classify(
                mut.kind, source, mut.content, current, route, route_reason
            )
            if hand == "confirmed-invalid":
                counters.kept_holes += 1
                if current != route:
                    counters.kept_disagree += 1
            elif hand == "known-gap-q7-mut":
                counters.kept_holes += 0  # tracked in totals only
            elif hand == "discarded-no-rule":
                counters.discarded_no_rule += 1
            else:
                counters.discarded_valid += 1

            if hand not in {"confirmed-invalid", "known-gap-q7-mut"}:
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
                    obligation.replace("\t", " "),
                    route_reason.replace("\t", " ")[:500],
                    review_reason.replace("\t", " ")[:500],
                ],
            )
            fixture_had_row = True

        if not fixture_had_row:
            append_tsv(
                tsv,
                [rel, "_no_mutations", "", "", "", "discarded-valid", "", "", "", "", ""],
            )

        with progress.open("a", encoding="utf-8") as pf:
            pf.write(rel + "\n")

        if counters.fixtures % 25 == 0:
            md.write_text(
                "\n".join(
                    _md_lines(
                        args.shard,
                        counters,
                        _holes_from_tsv(tsv),
                        _totals_from_tsv(tsv),
                    )
                ),
                encoding="utf-8",
            )

    holes_md = _holes_from_tsv(tsv)
    totals = _totals_from_tsv(tsv)
    md.write_text(
        "\n".join(_md_lines(args.shard, counters, holes_md, totals)),
        encoding="utf-8",
    )
    print(counters)
    return 0


def _totals_from_tsv(tsv: Path) -> dict:
    counts = {
        "mutants_run": 0,
        "confirmed_invalid": 0,
        "known_gap_q7": 0,
        "kept_disagree": 0,
        "discarded_valid": 0,
        "discarded_no_rule": 0,
        "mutant_run_failed": 0,
        "mutant_generate_failed": 0,
        "originals_skipped": 0,
        "confirmed_by_kind": {},
        "downgraded": 0,
        "baseline_mutants": 0,
    }
    if not tsv.exists():
        return counts
    from collections import Counter

    by_kind: Counter[str] = Counter()
    for line in tsv.read_text().splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) < 6:
            continue
        kind, hand, current, route = parts[1], parts[5], parts[3], parts[4]
        if kind == "_original_not_accepted":
            counts["originals_skipped"] += 1
            continue
        if kind == "_mutant_generate_failed":
            counts["mutant_generate_failed"] += 1
            counts["mutants_run"] += 1
            continue
        if kind.startswith("_"):
            continue
        if hand == "mutant-run-failed":
            counts["mutant_run_failed"] += 1
            counts["mutants_run"] += 1
            continue
        counts["mutants_run"] += 1
        if hand == "confirmed-invalid":
            counts["confirmed_invalid"] += 1
            by_kind[kind] += 1
            if current != route:
                counts["kept_disagree"] += 1
        elif hand == "known-gap-q7-mut":
            counts["known_gap_q7"] += 1
            by_kind[f"{kind} (q7)"] += 1
        elif hand == "discarded-no-rule":
            counts["discarded_no_rule"] += 1
        elif hand == "discarded-valid":
            counts["discarded_valid"] += 1
        else:
            counts["discarded_valid"] += 1
    counts["confirmed_by_kind"] = dict(by_kind)
    return counts


def _holes_from_tsv(tsv: Path) -> list[str]:
    rows: list[str] = []
    if not tsv.exists():
        return rows
    for line in tsv.read_text().splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) < 8 or parts[5] not in {"confirmed-invalid", "known-gap-q7-mut"}:
            continue
        rel, kind, mrel, current, route, hand, spec_path, spec_quote = parts[:8]
        obligation = parts[8] if len(parts) > 8 else ""
        rows.append(
            f"### {rel} — {kind}\n\n"
            f"- Original: `{rel}`\n"
            f"- Mutant: `{mrel}`\n"
            f"- Current checker: `{current}`; route: `{route}`\n"
            f"- Hand check: **{hand}**\n"
            f"- Broken obligation: `{obligation}`\n"
            f"- Spec ({spec_path}):\n\n"
            f"> {spec_quote}\n"
        )
    return rows


if __name__ == "__main__":
    sys.exit(main())
