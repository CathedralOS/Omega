#!/usr/bin/env python3
"""Job 2 soundness mutations runner for one shard. Audit-only; no compiler edits."""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
from mutant_classify import BUCKETS, classify_mutant  # noqa: E402

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = int(os.environ.get("SHARD", "3"))
SHARD_MOD = int(os.environ.get("SHARD_MOD", "8"))
TIMEOUT = int(os.environ.get("MUTATION_TIMEOUT", "120"))
OMEGA = Path(os.environ.get("OMEGA_BIN", REPO / "target/debug/omega"))
ROUTE = Path(
    os.environ.get(
        "ROUTE_BIN", REPO / "target/debug/examples/route_one_comparison"
    )
)

OUT_BASE = REPO / ".agents/four-jobs-2026-11-12" / "soundness"
MUTANT_ROOT = OUT_BASE / f"shard-{SHARD}"
TSV_PATH = OUT_BASE / f"shard-{SHARD}.tsv"
PROGRESS_PATH = OUT_BASE / f"shard-{SHARD}.progress.json"
MD_PATH = OUT_BASE / f"shard-{SHARD}.md"


@dataclass
class MutantRecord:
    original_fixture: str
    mutation_kind: str
    mutant_path: str
    current_verdict: str
    route_verdict: str
    hand_check: str
    disposition: str
    spec_path: str = ""
    spec_quote: str = ""
    spec_heading: str = ""
    obligation_line: str = ""
    seconds: float = 0.0


def list_shard_fixtures() -> list[str]:
    proc = subprocess.run(
        [
            "git",
            "-C",
            str(REPO),
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    lines = sorted(proc.stdout.splitlines())
    out = []
    for index, line in enumerate(lines):
        if index % SHARD_MOD == SHARD:
            out.append(line)
    return out


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def copy_fixture_package(fixture_main: Path, dest_root: Path) -> None:
    src_dir = fixture_main.parent
    if dest_root.exists():
        shutil.rmtree(dest_root)
    dest_root.mkdir(parents=True, exist_ok=True)
    for name in os.listdir(src_dir):
        if name == ".git" or name.startswith("."):
            continue
        src = src_dir / name
        dst = dest_root / name
        if src.is_dir():
            shutil.copytree(src, dst, dirs_exist_ok=True)
        else:
            shutil.copy2(src, dst)


def mutant_id(fixture: str, kind: str, suffix: str) -> str:
    digest = hashlib.sha1(f"{fixture}:{kind}:{suffix}".encode()).hexdigest()[:10]
    safe = fixture.replace("/", "__")
    return f"{safe}__{kind}__{digest}"


def generate_mutations(source: str) -> list[tuple[str, str]]:
    """Return (kind, new_source) pairs. Skip if pattern absent."""
    out: list[tuple[str, str]] = []

    def tighten_requires(match: re.Match[str]) -> str:
        lhs = match.group(0)
        num = re.search(r"==\s*(\d+)", lhs)
        if not num:
            return lhs
        n = int(num.group(1))
        return re.sub(r"==\s*\d+", f"== {n + 1}", lhs, count=1)

    if re.search(r"(?m)^\s*requires\b[^\n]*==\s*\d+", source):
        tightened = re.sub(
            r"(?m)^(\s*)requires\b[^\n]*==\s*\d+[^\n]*",
            lambda m: tighten_requires(m),
            source,
            count=1,
        )
        if tightened != source:
            out.append(("tighten_requires_equality", tightened))

    for binding in re.finditer(r"(?m)^\s*let mut ([A-Za-z_][A-Za-z0-9_]*)\b", source):
        name = binding.group(1)
        rest = source[binding.end() :]
        writes = re.search(
            rf"(?m)\b{re.escape(name)}\s*(\[|\.|=)|\b{re.escape(name)}\[[^\]]+\]\s*=",
            rest,
        )
        if writes:
            once = (
                source[: binding.start()]
                + binding.group(0).replace("let mut ", "let ", 1)
                + source[binding.end() :]
            )
            out.append(("remove_mut_binding", once))
            break

    if "&mut " in source:
        demoted = source.replace("&mut ", "&", 1)
        out.append(("demote_mut_ref_to_shared", demoted))

    shared_ref = re.search(r"(?<![&=])&(?![&=])(?=[A-Za-z_])", source)
    if shared_ref:
        promoted = source[: shared_ref.start()] + "&mut " + source[shared_ref.end() :]
        if promoted != source:
            out.append(("promote_shared_ref_to_mut", promoted))

    guard_line = re.search(
        r"(?m)^\s*transition\s*\{[^}]*\bif\b[^}]*\}", source
    )
    if not guard_line:
        guard_line = re.search(r"(?m)^\s*if\s+[^;{]+;\s*$", source)
    if guard_line:
        unguarded = source[: guard_line.start()] + source[guard_line.end() :]
        out.append(("delete_transition_guard", unguarded))

    call_mismatch = re.search(r"(?m)^(\s*\w+\()\d+(\)[;,])", source)
    if call_mismatch:
        corrupted_call = re.sub(
            r"(?m)^(\s*\w+\()\d+(\)[;,])",
            lambda m: f"{m.group(1)}0{m.group(2)}",
            source,
            count=1,
        )
        if corrupted_call != source:
            out.append(("corrupt_literal_callee_argument", corrupted_call))

    priority = [
        "remove_mut_binding",
        "demote_mut_ref_to_shared",
        "promote_shared_ref_to_mut",
        "tighten_requires_equality",
        "corrupt_literal_callee_argument",
        "delete_transition_guard",
    ]
    dedup: dict[str, str] = {}
    for kind, text in out:
        if text != source and text not in dedup.values():
            dedup[kind] = text
    ordered = [(kind, dedup[kind]) for kind in priority if kind in dedup]
    for kind, text in dedup.items():
        if kind not in priority:
            ordered.append((kind, text))
    return ordered[:4]


def run_omega_check(main_path: Path) -> tuple[str, float]:
    started = time.monotonic()
    try:
        proc = subprocess.run(
            [str(OMEGA), "--check", str(main_path)],
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
            stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        return "timeout", time.monotonic() - started
    except OSError as exc:
        return f"error:{exc}", time.monotonic() - started

    if proc.returncode == 0:
        return "accepted", time.monotonic() - started
    if proc.returncode == 1:
        return "rejected", time.monotonic() - started
    if proc.returncode == 2:
        return "incomplete", time.monotonic() - started
    return f"exit_{proc.returncode}", time.monotonic() - started


def run_route(main_paths: list[Path]) -> dict[str, tuple[str, str, float]]:
    """Map main path -> (current, route, seconds)."""
    if not main_paths:
        return {}
    started = time.monotonic()
    try:
        proc = subprocess.run(
            [str(ROUTE), *[str(p) for p in main_paths]],
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=TIMEOUT * max(1, len(main_paths)),
            stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        elapsed = time.monotonic() - started
        return {str(p): ("timeout", "timeout", elapsed) for p in main_paths}

    elapsed = time.monotonic() - started
    per = elapsed / max(len(main_paths), 1)
    results: dict[str, tuple[str, str, float]] = {}
    for line in proc.stdout.splitlines():
        if not line.startswith("program\t"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        path = parts[1]
        tail = parts[2]
        current = route = "unknown"
        for token in tail.split():
            if token.startswith("current="):
                current = token.split("=", 1)[1]
            if token.startswith("route="):
                route = token.split("=", 1)[1]
        results[path] = (current, route, per)
    for path in main_paths:
        results.setdefault(str(path), ("unknown", "unknown", per))
    return results


def load_progress() -> set[str]:
    if not PROGRESS_PATH.exists():
        return set()
    data = json.loads(PROGRESS_PATH.read_text())
    return set(data.get("done_fixtures", []))


def save_progress(done: set[str], counts: dict[str, int]) -> None:
    PROGRESS_PATH.write_text(
        json.dumps(
            {
                "pin": PIN,
                "shard": SHARD,
                "done_fixtures": sorted(done),
                "counts": counts,
            },
            indent=2,
        )
        + "\n"
    )


TSV_HEADER = (
    "original_fixture\tmutation_kind\tmutant_path\tcurrent_verdict\t"
    "route_verdict\thand_check\tdisposition\tspec_heading\tspec_path\t"
    "obligation_line\tseconds\n"
)


def append_tsv(rows: list[MutantRecord]) -> None:
    if not TSV_PATH.exists():
        TSV_PATH.write_text(TSV_HEADER)
    with TSV_PATH.open("a", encoding="utf-8") as handle:
        for row in rows:
            handle.write(
                f"{row.original_fixture}\t{row.mutation_kind}\t{row.mutant_path}\t"
                f"{row.current_verdict}\t{row.route_verdict}\t{row.hand_check}\t"
                f"{row.disposition}\t{row.spec_heading}\t{row.spec_path}\t"
                f"{row.obligation_line}\t{row.seconds:.3f}\n"
            )


def bucket_counts_from_rows(rows: list[MutantRecord]) -> dict[str, int]:
    counts = {bucket: 0 for bucket in BUCKETS}
    for row in rows:
        counts[row.disposition] = counts.get(row.disposition, 0) + 1
    return counts


def parse_tsv_line(cols: list[str]) -> MutantRecord | None:
    if len(cols) < 7:
        return None
    if len(cols) >= 11:
        return MutantRecord(
            original_fixture=cols[0],
            mutation_kind=cols[1],
            mutant_path=cols[2],
            current_verdict=cols[3],
            route_verdict=cols[4],
            hand_check=cols[5],
            disposition=cols[6],
            spec_heading=cols[7],
            spec_path=cols[8],
            obligation_line=cols[9],
            seconds=float(cols[10]) if cols[10] else 0.0,
        )
    return MutantRecord(
        original_fixture=cols[0],
        mutation_kind=cols[1],
        mutant_path=cols[2],
        current_verdict=cols[3],
        route_verdict=cols[4],
        hand_check=cols[5],
        disposition=cols[6],
        spec_path=cols[7] if len(cols) > 7 else "",
        seconds=float(cols[8]) if len(cols) > 8 and cols[8] else 0.0,
    )


def revalidate_tsv_rows() -> list[MutantRecord]:
    if not TSV_PATH.exists():
        return []
    rows: list[MutantRecord] = []
    for line in TSV_PATH.read_text(encoding="utf-8").splitlines()[1:]:
        cols = line.split("\t")
        old = parse_tsv_line(cols)
        if not old:
            continue
        original_path = REPO / old.original_fixture
        mutant_path = Path(old.mutant_path)
        if not original_path.is_file() or not mutant_path.is_file():
            rows.append(old)
            continue
        original = read_text(original_path)
        mutant = read_text(mutant_path)
        result = classify_mutant(
            old.mutation_kind,
            old.current_verdict,
            old.route_verdict,
            original,
            mutant,
        )
        spec_path = spec_quote = spec_heading = ""
        if result.spec:
            spec_path = result.spec.path
            spec_quote = result.spec.quote
            spec_heading = result.spec.heading
        rows.append(
            MutantRecord(
                original_fixture=old.original_fixture,
                mutation_kind=old.mutation_kind,
                mutant_path=old.mutant_path,
                current_verdict=old.current_verdict,
                route_verdict=old.route_verdict,
                hand_check=result.hand_check,
                disposition=result.disposition,
                spec_path=spec_path,
                spec_quote=spec_quote,
                spec_heading=spec_heading,
                obligation_line=result.obligation_line,
                seconds=old.seconds,
            )
        )
    TSV_PATH.write_text(TSV_HEADER)
    append_tsv(rows)
    return rows


def capture_check_output(main_path: Path) -> tuple[int, str, str]:
    proc = subprocess.run(
        [str(OMEGA), "--check", str(main_path)],
        cwd=REPO,
        capture_output=True,
        text=True,
        timeout=TIMEOUT,
        stdin=subprocess.DEVNULL,
    )
    return proc.returncode, proc.stdout[:2000], proc.stderr[:2000]


def capture_route_output(main_path: Path) -> str:
    proc = subprocess.run(
        [str(ROUTE), str(main_path)],
        cwd=REPO,
        capture_output=True,
        text=True,
        timeout=TIMEOUT,
        stdin=subprocess.DEVNULL,
    )
    return "\n".join(proc.stdout.splitlines()[:30])


def write_md(rows: list[MutantRecord], meta: dict[str, int], host: str) -> None:
    buckets = bucket_counts_from_rows(rows)
    mutants_run = len(rows)
    bucket_sum = sum(buckets.get(b, 0) for b in BUCKETS)
    confirmed = [row for row in rows if row.disposition == "confirmed-invalid"]
    by_kind: dict[str, int] = {}
    for row in confirmed:
        by_kind[row.mutation_kind] = by_kind.get(row.mutation_kind, 0) + 1

    lines = [
        f"# Soundness mutations shard {SHARD}",
        "",
        f"PIN: `{PIN}` (`origin/main` at launch).",
        f"Shard selection: `(i % {SHARD_MOD}) == {SHARD}` over sorted "
        "`tests/omega/pass/**/main.omg` and `tests/omega/run/**/main.omg`.",
        "",
        "## Mutant buckets (each row is exactly one bucket)",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for bucket in BUCKETS:
        lines.append(f"| `{bucket}` | {buckets.get(bucket, 0)} |")
    lines.extend(
        [
            f"| **Total (mutants run)** | **{mutants_run}** |",
            "",
            f"Bucket sum check: {bucket_sum} == {mutants_run} ({ 'OK' if bucket_sum == mutants_run else 'MISMATCH'}).",
            "",
            f"- Fixtures in shard: {meta.get('fixtures', 0)}",
            f"- Fixtures processed: {meta.get('fixtures_processed', 0)}",
            f"- Mutants generated: {meta.get('mutants_generated', mutants_run)}",
            "",
            "### Confirmed-invalid by mutation kind",
            "",
        ]
    )
    if by_kind:
        for kind, count in sorted(by_kind.items(), key=lambda item: (-item[1], item[0])):
            lines.append(f"- `{kind}`: {count}")
    else:
        lines.append("_None._")
    lines.extend(
        [
            "",
            "## Commands",
            "",
            "```bash",
            f"git checkout {PIN}",
            "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target "
            "mbx build -p omega -p compiler --example route_one_comparison < /dev/null",
            f"SHARD={SHARD} python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py",
            "SHARD=3 python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py --revalidate",
            "```",
            "",
            f"Harness build commit: `{PIN}`. Host: `{host}`.",
            f"Omega: `{OMEGA}`. Route: `{ROUTE}`. Per-mutant timeout: {TIMEOUT}s.",
            "",
            "## Hand verification (5 confirmed-invalid)",
            "",
        ]
    )
    samples = confirmed[:5]
    if not samples:
        lines.append("_No confirmed-invalid rows to sample yet._")
    for index, row in enumerate(samples, 1):
        exit_code, out, err = capture_check_output(Path(row.mutant_path))
        route_out = capture_route_output(Path(row.mutant_path))
        original = read_text(REPO / row.original_fixture)
        mutant = read_text(Path(row.mutant_path))
        lines.extend(
            [
                f"### Sample {index}: `{row.original_fixture}` / `{row.mutation_kind}`",
                "",
                f"- Obligation line: `{row.obligation_line}`",
                f"- Disposition: `{row.disposition}`",
                f"- Current verdict: **{row.current_verdict}** (exit {exit_code})",
                f"- Route verdict: **{row.route_verdict}**",
                "",
                f"**Spec** — `{row.spec_path}` ({row.spec_heading}):",
                "",
                f"> {row.spec_quote}",
                "",
                "**omega --check** (stdout/stderr head):",
                "",
                "```text",
                out.strip() or "(empty)",
                err.strip() or "",
                "```",
                "",
                "**route_one_comparison** (first lines):",
                "",
                "```text",
                route_out.strip() or "(empty)",
                "```",
                "",
            ]
        )
    lines.extend(["## Confirmed-invalid entries", ""])
    if not confirmed:
        lines.append("_None._")
    else:
        for index, row in enumerate(confirmed, 1):
            lines.extend(
                [
                    f"### {index}. `{row.original_fixture}` — `{row.mutation_kind}`",
                    "",
                    f"- Mutant: `{row.mutant_path}`",
                    f"- Current: **{row.current_verdict}** | Route: **{row.route_verdict}**",
                    f"- Obligation: `{row.obligation_line}`",
                    "",
                    f"**{row.spec_path}** — *{row.spec_heading}*:",
                    "",
                    f"> {row.spec_quote}",
                    "",
                ]
            )
    lines.extend(["## Known gap: Q7 mut", ""])
    q7 = [row for row in rows if row.disposition == "known-gap-q7-mut"]
    lines.append(f"Count: {len(q7)} (tracked migration; not counted as confirmed holes).")
    MD_PATH.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    if not OMEGA.is_file() or not ROUTE.is_file():
        print("Build omega and route_one_comparison first.", file=sys.stderr)
        return 2

    if "--revalidate" in sys.argv:
        rows = revalidate_tsv_rows()
        meta = json.loads(PROGRESS_PATH.read_text())["counts"] if PROGRESS_PATH.exists() else {}
        meta.setdefault("fixtures_processed", len(load_progress()))
        host = subprocess.check_output(["hostname"], text=True).strip()
        write_md(rows, meta, host)
        print(json.dumps({"revalidated": len(rows), "buckets": bucket_counts_from_rows(rows)}, indent=2))
        return 0

    fixtures = list_shard_fixtures()
    done = load_progress()
    counts = {
        "fixtures": len(fixtures),
        "fixtures_processed": len(done),
        "mutants_generated": 0,
    }
    all_rows: list[MutantRecord] = []
    if TSV_PATH.exists():
        all_rows = [
            parse_tsv_line(line.split("\t"))
            for line in TSV_PATH.read_text().splitlines()[1:]
            if parse_tsv_line(line.split("\t"))
        ]
        counts["mutants_generated"] = len(all_rows)

    for fixture in fixtures:
        if fixture in done:
            continue
        main_src = REPO / fixture
        source = read_text(main_src)
        mutations = generate_mutations(source)
        counts["mutants_generated"] += len(mutations)
        batch_dirs: list[Path] = []
        batch_meta: list[tuple[str, Path]] = []

        for kind, mutant_source in mutations:
            mid = mutant_id(fixture, kind, mutant_source[:80])
            dest = MUTANT_ROOT / mid
            copy_fixture_package(main_src, dest)
            write_text(dest / "main.omg", mutant_source)
            main_path = dest / "main.omg"
            batch_dirs.append(main_path)
            batch_meta.append((kind, main_path))

        current_by_path: dict[str, str] = {}
        with ThreadPoolExecutor(max_workers=4) as pool:
            futures = {
                pool.submit(run_omega_check, main_path): (kind, main_path)
                for kind, main_path in batch_meta
            }
            for future in as_completed(futures):
                kind, main_path = futures[future]
                verdict, _seconds = future.result()
                current_by_path[str(main_path)] = verdict
                counts["mutants_run"] += 1

        route_results = run_route(batch_dirs)

        fixture_rows: list[MutantRecord] = []
        original = source
        for kind, main_path in batch_meta:
            key = str(main_path)
            current = current_by_path.get(key, "unknown")
            route, _, rsec = route_results.get(key, ("unknown", "unknown", 0.0))
            mutant = read_text(main_path)
            result = classify_mutant(kind, current, route, original, mutant)
            spec_path = spec_quote = spec_heading = ""
            if result.spec:
                spec_path = result.spec.path
                spec_quote = result.spec.quote
                spec_heading = result.spec.heading
            row = MutantRecord(
                original_fixture=fixture,
                mutation_kind=kind,
                mutant_path=key,
                current_verdict=current,
                route_verdict=route,
                hand_check=result.hand_check,
                disposition=result.disposition,
                spec_path=spec_path,
                spec_quote=spec_quote,
                spec_heading=spec_heading,
                obligation_line=result.obligation_line,
                seconds=rsec,
            )
            fixture_rows.append(row)

        append_tsv(fixture_rows)
        all_rows.extend(fixture_rows)
        counts["mutants_generated"] = len(all_rows)
        done.add(fixture)
        counts["fixtures_processed"] = len(done)
        save_progress(done, counts)

        if counts["fixtures_processed"] % 25 == 0:
            host = subprocess.check_output(["hostname"], text=True).strip()
            write_md(all_rows, counts, host)

    all_rows = revalidate_tsv_rows()
    host = subprocess.check_output(["hostname"], text=True).strip()
    write_md(all_rows, counts, host)
    print(
        json.dumps(
            {
                **counts,
                "mutants_run": len(all_rows),
                "buckets": bucket_counts_from_rows(all_rows),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
