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
from mutant_classify import (  # noqa: E402
    BUCKETS,
    LEDGER_DISPOSITIONS,
    SPEC_RULES,
    canonical_kind,
    classify_mutant,
    drop_requires_removed_line,
    likely_runtime_trapped,
    minimal_mutant_diff,
)

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
MUTANT_ROOT = Path(
    os.environ.get(
        "MUTANT_ROOT",
        f"/tmp/omega-soundness-mutations/shard-{SHARD}",
    )
)
TSV_PATH = OUT_BASE / f"shard-{SHARD}.tsv"
PROGRESS_PATH = OUT_BASE / f"shard-{SHARD}.progress.json"
MD_PATH = OUT_BASE / f"shard-{SHARD}.md"
STATS_PATH = OUT_BASE / f"shard-{SHARD}.stats.json"


@dataclass
class MutantRecord:
    original_fixture: str
    mutation_kind: str
    mutant_path: str
    current_verdict: str
    route_verdict: str
    hand_check: str
    disposition: str
    kind: str = ""
    spec_path: str = ""
    spec_quote: str = ""
    spec_heading: str = ""
    obligation_line: str = ""
    route_reason: str = ""
    accepted_by: str = ""
    validity_note: str = ""
    hole_subtype: str = ""
    minimal_diff: str = ""
    no_runtime_trap_evidence: str = ""
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


def drop_requires_variants(source: str) -> list[str]:
    """Drop one body requires line and/or machine-header `requires` clauses."""
    variants: list[str] = []
    requires_lines = list(re.finditer(r"(?m)^(\s*)requires\b[^\n]*\n", source))
    if requires_lines:
        first = requires_lines[0]
        stripped = source[: first.start()] + source[first.end() :]
        if stripped != source:
            variants.append(stripped)
    header_pat = re.compile(
        r"(?m)^(\s*(?:\w+(?:_\w+)*\s+)*)machine\s+([^\n{]+?)\s+requires\s+"
        r"([^;{]+?)(\s*\{[^\n]*)"
    )
    for match in header_pat.finditer(source):
        prefix = match.group(1) or ""
        signature = match.group(2).strip()
        brace_tail = match.group(4)
        new_line = f"{prefix}machine {signature}{brace_tail}"
        mutant = source[: match.start()] + new_line + source[match.end() :]
        if mutant != source and mutant not in variants:
            variants.append(mutant)
    return variants


def drops_machine_header_requires(source: str, variant: str) -> bool:
    variant_lines = {line.strip() for line in variant.splitlines()}
    for line in source.splitlines():
        stripped = line.strip()
        if not re.search(r"\bmachine\b", stripped) or " requires " not in stripped:
            continue
        if stripped in variant_lines:
            continue
        without = re.sub(r"\s+requires\s+.+?(?=\s*\{)", "", stripped).strip()
        if without in variant_lines:
            return True
    return False


def drop_requires_backfill_pending(
    fixture: str, source: str, existing_paths: set[str]
) -> list[tuple[str, str]]:
    variants = drop_requires_variants(source)
    if not variants:
        return []
    fixture_prefix = fixture.replace("/", "__")
    has_drop = any(
        fixture_prefix in path and "drop_requires_line" in path for path in existing_paths
    )
    pending: list[tuple[str, str]] = []
    for variant in variants:
        mid = mutant_id(fixture, "drop_requires_line", variant[:80])
        main_path = str(MUTANT_ROOT / mid / "main.omg")
        if main_path in existing_paths or Path(main_path).is_file():
            continue
        if not has_drop and variant == variants[0]:
            pending.append(("drop_requires_line", variant))
            continue
        if has_drop and drops_machine_header_requires(source, variant):
            pending.append(("drop_requires_line", variant))
    return pending


def describe_no_mutation_reason(source: str) -> str:
    notes: list[str] = []
    if not drop_requires_variants(source):
        notes.append("no requires clause to drop (body or machine header)")
    if not re.search(r"(?m)^\s*let mut ", source):
        notes.append("no let mut binding with downstream write")
    if "&mut " not in source:
        notes.append("no &mut reference to demote")
    if not re.search(r"(?<![&=])&(?![&=])(?=[A-Za-z_])", source):
        notes.append("no shared & reference to promote")
    if not re.search(r"(?m)^\s*requires\b[^\n]*==\s*\d+", source):
        notes.append("no numeric requires to tighten")
    if not re.search(r"(?m)^\s*\w+\(\d+\)", source):
        notes.append("no literal call argument to corrupt")
    return "; ".join(notes) if notes else "generate_mutations produced no applicable edit"


def no_runtime_trap_evidence(mutant: str, disposition: str) -> str:
    if disposition != "confirmed-hole":
        return ""
    if likely_runtime_trapped(mutant):
        return "FAIL: explicit bad()/crash route present in mutant"
    return "no bad()/crash route; violation is compile-time proof obligation"


def generate_mutations(source: str) -> list[tuple[str, str]]:
    """Return (kind, new_source) pairs. Skip if pattern absent."""
    out: list[tuple[str, str]] = []

    drops = drop_requires_variants(source)
    if drops:
        out.append(("drop_requires_line", drops[0]))

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
        "drop_requires_line",
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
    return ordered[:5]


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


def run_route(main_paths: list[Path]) -> dict[str, tuple[str, str, float, str]]:
    """Map main path -> (route_current, route_verdict, seconds, reason)."""
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
        return {
            str(p): ("timeout", "timeout", elapsed, "timeout") for p in main_paths
        }

    elapsed = time.monotonic() - started
    per = elapsed / max(len(main_paths), 1)
    reasons: dict[str, list[str]] = {str(p): [] for p in main_paths}
    results: dict[str, tuple[str, str, float, str]] = {}
    for line in proc.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) < 2:
            continue
        kind = parts[0]
        path = parts[1]
        if kind in ("diagnostic", "fallback", "use") and len(parts) >= 6:
            reasons.setdefault(path, []).append(" ".join(parts[2:6])[:240])
        if not line.startswith("program\t"):
            continue
        if len(parts) < 3:
            continue
        tail = parts[2]
        current = route = "unknown"
        for token in tail.split():
            if token.startswith("current="):
                current = token.split("=", 1)[1]
            if token.startswith("route="):
                route = token.split("=", 1)[1]
        reason = "; ".join(reasons.get(path, [])[:2]) or f"route={route}"
        results[path] = (current, route, per, reason)
    for path in main_paths:
        key = str(path)
        if key not in results:
            reason = "; ".join(reasons.get(key, [])[:2]) or "route=unknown"
            results[key] = ("unknown", "unknown", per, reason)
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
    "original_fixture\tmutation_kind\tkind\tmutant_path\tcurrent_verdict\t"
    "route_verdict\thand_check\tdisposition\taccepted_by\tspec_heading\tspec_path\t"
    "obligation_line\troute_reason\tvalidity_note\thole_subtype\tspec_quote\t"
    "minimal_diff\tno_runtime_trap_evidence\tseconds\n"
)


def append_tsv(rows: list[MutantRecord]) -> None:
    if not TSV_PATH.exists():
        TSV_PATH.write_text(TSV_HEADER)
    with TSV_PATH.open("a", encoding="utf-8") as handle:
        for row in rows:
            kind = row.kind or canonical_kind(row.mutation_kind)
            handle.write(
                f"{row.original_fixture}\t{row.mutation_kind}\t{kind}\t{row.mutant_path}\t"
                f"{row.current_verdict}\t{row.route_verdict}\t{row.hand_check}\t"
                f"{row.disposition}\t{row.accepted_by}\t{row.spec_heading}\t{row.spec_path}\t"
                f"{row.obligation_line}\t{row.route_reason}\t{row.validity_note}\t"
                f"{row.hole_subtype}\t{_tsv_cell(row.spec_quote)}\t"
                f"{_tsv_cell(row.minimal_diff)}\t{_tsv_cell(row.no_runtime_trap_evidence)}\t"
                f"{row.seconds:.3f}\n"
            )


def _tsv_cell(text: str) -> str:
    return text.replace("\t", " ").replace("\r", "").replace("\n", "\\n")


def spec_quote_for_kind(kind: str) -> str:
    rule = SPEC_RULES.get(kind)
    return rule.quote if rule else ""


def probe_drop_requires_subtype(mutant_path: Path, mutant: str, removed: str) -> str:
    """Violating caller outside the machine: inferred-entry vs unchecked; else probe-not-built."""
    from mutant_classify import internal_restates_removed

    if not removed:
        return "probe-not-built"
    if internal_restates_removed(removed, mutant)[0]:
        return ""
    calls = [
        match
        for match in re.findall(r"(?m)^\s+([A-Za-z_][\w]*)\(", mutant)
        if match not in {"machine", "data", "use", "let"}
    ]
    if not calls:
        return "probe-not-built"
    target = calls[0]
    numeric = re.search(r"==\s*(\d+)", removed)
    if not numeric:
        return "probe-not-built"
    bad = "0"
    probe_src = mutant.rstrip() + f"\n\nmachine __violation_probe__() {{\n    {target}({bad});\n}}\n"
    probe_file = mutant_path.parent / "main.probe-violation.omg"
    try:
        probe_file.write_text(probe_src, encoding="utf-8")
        mutant_ok, _ = run_omega_check(mutant_path)
        probe_ok, _ = run_omega_check(probe_file)
    except OSError:
        return "probe-not-built"
    finally:
        probe_file.unlink(missing_ok=True)
    if mutant_ok == "accepted" and probe_ok == "rejected":
        return "inferred-entry"
    if mutant_ok == "accepted" and probe_ok == "accepted":
        return "unchecked"
    return "probe-not-built"


def is_mutant_row(row: MutantRecord) -> bool:
    return row.disposition not in LEDGER_DISPOSITIONS and row.mutation_kind != "(none)"


def bucket_counts_from_rows(rows: list[MutantRecord]) -> dict[str, int]:
    counts = {bucket: 0 for bucket in BUCKETS}
    for row in rows:
        if not is_mutant_row(row):
            continue
        counts[row.disposition] = counts.get(row.disposition, 0) + 1
    return counts


def fixture_ledger_counts(
    rows: list[MutantRecord], shard_fixtures: int
) -> dict[str, int]:
    fixtures_with_mutants = {
        row.original_fixture for row in rows if is_mutant_row(row)
    }
    ledger = {
        row.original_fixture: row.disposition
        for row in rows
        if row.disposition in LEDGER_DISPOSITIONS
    }
    no_mutation = sum(1 for value in ledger.values() if value == "no-mutation")
    failed_run = sum(1 for value in ledger.values() if value == "failed-run")
    original_not = sum(
        1 for value in ledger.values() if value == "original-not-accepted"
    )
    mutated = len(fixtures_with_mutants)
    return {
        "shard_fixtures": shard_fixtures,
        "mutated": mutated,
        "no_mutation": no_mutation,
        "failed_run": failed_run,
        "original_not_accepted": original_not,
        "ledger_rows": no_mutation + failed_run + original_not,
    }


def build_record(
    *,
    fixture: str,
    kind: str,
    mutant_path: str,
    current: str,
    route: str,
    result,
    source: str,
    mutant: str,
    route_reason: str,
    hole_subtype: str,
    seconds: float,
) -> MutantRecord:
    spec_path = spec_quote = spec_heading = ""
    if result.spec:
        spec_path = result.spec.path
        spec_quote = result.spec.quote
        spec_heading = result.spec.heading
    disposition = result.disposition
    diff = minimal_mutant_diff(source, mutant) if mutant_path and mutant else ""
    return MutantRecord(
        original_fixture=fixture,
        mutation_kind=kind,
        kind=canonical_kind(kind),
        mutant_path=mutant_path,
        current_verdict=current,
        route_verdict=route,
        hand_check=result.hand_check,
        disposition=disposition,
        spec_path=spec_path,
        spec_quote=spec_quote if disposition == "confirmed-hole" else "",
        spec_heading=spec_heading,
        obligation_line=result.obligation_line,
        route_reason=result.route_reason or route_reason,
        accepted_by=result.accepted_by,
        validity_note=result.validity_note,
        hole_subtype=(
            "n/a: modular-entry probe not applicable (literal vs callee requires)"
            if disposition == "confirmed-hole"
            and kind == "corrupt_literal_callee_argument"
            and not hole_subtype
            else hole_subtype
        ),
        minimal_diff=diff if disposition == "confirmed-hole" else "",
        no_runtime_trap_evidence=no_runtime_trap_evidence(mutant, disposition)
        if mutant
        else "",
        seconds=seconds,
    )


def ensure_fixture_ledger(
    rows: list[MutantRecord], fixtures: list[str]
) -> list[MutantRecord]:
    mutant_rows = [row for row in rows if is_mutant_row(row)]
    ledger_rows = [row for row in rows if row.disposition in LEDGER_DISPOSITIONS]
    fixtures_with_mutants = {row.original_fixture for row in mutant_rows}
    ledger_by_fixture = {row.original_fixture: row for row in ledger_rows}
    for fixture in fixtures:
        if fixture in fixtures_with_mutants or fixture in ledger_by_fixture:
            continue
        main_path = REPO / fixture
        current, seconds = run_omega_check(main_path)
        if current in ("rejected", "stopped", "incomplete"):
            ledger_by_fixture[fixture] = MutantRecord(
                original_fixture=fixture,
                mutation_kind="(none)",
                kind="(none)",
                mutant_path=str(main_path),
                current_verdict=current,
                route_verdict="n/a",
                hand_check="fixture-ledger",
                disposition="original-not-accepted",
                validity_note=(
                    f"original fixture --check verdict `{current}` at PIN harness "
                    "(expected for negative/stopped corpus rows; not a harness error)"
                ),
                seconds=seconds,
            )
            continue
        if current in ("timeout",) or current.startswith("exit_") or current == "error":
            ledger_by_fixture[fixture] = MutantRecord(
                original_fixture=fixture,
                mutation_kind="(none)",
                kind="(none)",
                mutant_path=str(main_path),
                current_verdict=current,
                route_verdict="n/a",
                hand_check="fixture-ledger",
                disposition="failed-run",
                validity_note=f"fixture harness error: omega --check {current} at PIN",
                seconds=seconds,
            )
            continue
        if current != "accepted":
            ledger_by_fixture[fixture] = MutantRecord(
                original_fixture=fixture,
                mutation_kind="(none)",
                kind="(none)",
                mutant_path=str(main_path),
                current_verdict=current,
                route_verdict="n/a",
                hand_check="fixture-ledger",
                disposition="failed-run",
                validity_note=f"fixture harness unexpected verdict `{current}` at PIN",
                seconds=seconds,
            )
            continue
        source = read_text(main_path)
        ledger_by_fixture[fixture] = MutantRecord(
            original_fixture=fixture,
            mutation_kind="(none)",
            kind="(none)",
            mutant_path=str(main_path),
            current_verdict="accepted",
            route_verdict="n/a",
            hand_check="fixture-ledger",
            disposition="no-mutation",
            validity_note=describe_no_mutation_reason(source),
            seconds=seconds,
        )
    return mutant_rows + sorted(
        ledger_by_fixture.values(), key=lambda row: row.original_fixture
    )


def _unescape_tsv_cell(text: str) -> str:
    return (text or "").replace("\\n", "\n")


def parse_tsv_line(cols: list[str]) -> MutantRecord | None:
    if len(cols) < 7:
        return None
    legacy_path_second = cols[2].startswith("/") or cols[2].startswith("tests/")
    if len(cols) >= 16 and not legacy_path_second:
        return MutantRecord(
            original_fixture=cols[0],
            mutation_kind=cols[1],
            kind=cols[2] or canonical_kind(cols[1]),
            mutant_path=cols[3],
            current_verdict=cols[4],
            route_verdict=cols[5],
            hand_check=cols[6],
            disposition=cols[7],
            accepted_by=cols[8],
            spec_heading=cols[9],
            spec_path=cols[10],
            obligation_line=cols[11],
            route_reason=cols[12],
            validity_note=cols[13] if len(cols) > 13 else "",
            hole_subtype=cols[14] if len(cols) > 14 else "",
            spec_quote=_unescape_tsv_cell(cols[15] if len(cols) > 15 else "")
            or spec_quote_for_kind(cols[1]),
            minimal_diff=_unescape_tsv_cell(cols[16] if len(cols) > 16 else ""),
            no_runtime_trap_evidence=cols[17] if len(cols) > 17 else "",
            seconds=float(cols[18]) if len(cols) > 18 and cols[18] else 0.0,
        )
    if len(cols) >= 14:
        return MutantRecord(
            original_fixture=cols[0],
            mutation_kind=cols[1],
            kind=canonical_kind(cols[1]),
            mutant_path=cols[2],
            current_verdict=cols[3],
            route_verdict=cols[4],
            hand_check=cols[5],
            disposition=cols[6],
            accepted_by=cols[7],
            spec_heading=cols[8],
            spec_path=cols[9],
            obligation_line=cols[10],
            route_reason=cols[11],
            validity_note=cols[12] if len(cols) > 12 else "",
            hole_subtype=cols[13] if len(cols) > 13 else "",
            spec_quote=spec_quote_for_kind(cols[1]),
            seconds=float(cols[14]) if len(cols) > 14 and cols[14] else 0.0,
        )
    return None


def revalidate_tsv_rows() -> list[MutantRecord]:
    if not TSV_PATH.exists():
        return ensure_fixture_ledger([], list_shard_fixtures())
    mutant_rows: list[MutantRecord] = []
    for line in TSV_PATH.read_text(encoding="utf-8").splitlines()[1:]:
        cols = line.split("\t")
        old = parse_tsv_line(cols)
        if not old or not is_mutant_row(old):
            continue
        original_path = REPO / old.original_fixture
        mutant_path = Path(old.mutant_path)
        if not original_path.is_file() or not mutant_path.is_file():
            mutant_rows.append(old)
            continue
        original = read_text(original_path)
        mutant = read_text(mutant_path)
        route_reason = old.route_reason
        if old.mutation_kind == "drop_requires_line" and not route_reason:
            route_reason = run_route([mutant_path]).get(
                str(mutant_path), ("", "", 0.0, "")
            )[3]
        result = classify_mutant(
            old.mutation_kind,
            old.current_verdict,
            old.route_verdict,
            original,
            mutant,
            route_reason=route_reason,
        )
        hole_subtype = result.hole_subtype
        if (
            old.mutation_kind == "drop_requires_line"
            and result.disposition == "confirmed-hole"
            and result.accepted_by
        ):
            removed = drop_requires_removed_line(original, mutant)
            hole_subtype = probe_drop_requires_subtype(mutant_path, mutant, removed)
        mutant_rows.append(
            build_record(
                fixture=old.original_fixture,
                kind=old.mutation_kind,
                mutant_path=old.mutant_path,
                current=old.current_verdict,
                route=old.route_verdict,
                result=result,
                source=original,
                mutant=mutant,
                route_reason=route_reason,
                hole_subtype=hole_subtype,
                seconds=old.seconds,
            )
        )
    fixtures = list_shard_fixtures()
    all_rows = ensure_fixture_ledger(mutant_rows, fixtures)
    TSV_PATH.write_text(TSV_HEADER)
    append_tsv(all_rows)
    return all_rows


def integrity_report(mutant_rows: list[MutantRecord]) -> dict[str, object]:
    from mutant_classify import route_outside, route_rejects

    rejected_both = [
        row for row in mutant_rows if row.disposition == "rejected-both"
    ]
    rb_bad = [
        row
        for row in rejected_both
        if not (route_rejects(row.current_verdict) and route_rejects(row.route_verdict))
    ]
    header_backfill_buckets: dict[str, int] = {}
    header_drop_rows = 0
    for row in mutant_rows:
        if row.mutation_kind != "drop_requires_line":
            continue
        path = Path(row.mutant_path)
        original = read_text(REPO / row.original_fixture)
        if path.is_file():
            mutant = read_text(path)
            is_header = drops_machine_header_requires(original, mutant)
        else:
            diff = row.minimal_diff or ""
            is_header = bool(
                re.search(r"(?m)^-.*\bmachine\b.*\brequires\b", diff)
            )
        if is_header:
            header_drop_rows += 1
            header_backfill_buckets[row.disposition] = (
                header_backfill_buckets.get(row.disposition, 0) + 1
            )
    return {
        "rejected_both_total": len(rejected_both),
        "rejected_both_not_dual_reject": len(rb_bad),
        "rejected_both_bad_route_verdicts": dict(
            __import__("collections").Counter(
                row.route_verdict for row in rb_bad
            )
        ),
        "valid_route_outside_count": sum(
            1 for row in mutant_rows if row.disposition == "valid-route-outside"
        ),
        "rejected_current_route_outside_count": sum(
            1
            for row in mutant_rows
            if row.disposition == "rejected-current-route-outside"
        ),
        "header_drop_requires_rows": header_drop_rows,
        "header_drop_requires_by_bucket": header_backfill_buckets,
    }


def write_stats(rows: list[MutantRecord], meta: dict[str, int]) -> None:
    mutant_rows = [row for row in rows if is_mutant_row(row)]
    drop_rows = [
        row for row in mutant_rows if row.mutation_kind == "drop_requires_line"
    ]
    payload = {
        "pin": PIN,
        "shard": SHARD,
        "tsv_rows": len(rows),
        "mutant_rows": len(mutant_rows),
        "buckets": bucket_counts_from_rows(rows),
        "fixture_ledger": fixture_ledger_counts(
            rows, meta.get("fixtures", len(list_shard_fixtures()))
        ),
        "integrity": integrity_report(mutant_rows),
        "drop_requires_rows": len(drop_rows),
        "backfill_drop_requires_rows": max(len(drop_rows) - 347, 0),
        "mutant_root": str(MUTANT_ROOT),
        "repo_tracked_hint": (
            "Mutant trees and probe files are written under MUTANT_ROOT (/tmp); "
            "git tracks only scripts, TSV, md, stats, progress."
        ),
    }
    STATS_PATH.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


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
    mutant_rows = [row for row in rows if is_mutant_row(row)]
    buckets = bucket_counts_from_rows(rows)
    mutants_run = len(mutant_rows)
    bucket_sum = sum(buckets.get(b, 0) for b in BUCKETS)
    shard_fixtures = meta.get("fixtures", len(list_shard_fixtures()))
    ledger = fixture_ledger_counts(rows, shard_fixtures)
    rows_by_fixture: dict[str, list[MutantRecord]] = {}
    for row in mutant_rows:
        rows_by_fixture.setdefault(row.original_fixture, []).append(row)
    mutated_fixtures = len(rows_by_fixture)
    no_mutation = ledger["no_mutation"]
    multi_row = [
        (fixture, len(items))
        for fixture, items in sorted(rows_by_fixture.items())
        if len(items) > 1
    ]
    confirmed = [row for row in mutant_rows if row.disposition == "confirmed-hole"]
    by_kind: dict[str, int] = {}
    by_canonical: dict[str, int] = {}
    by_accepted: dict[str, int] = {}
    by_subtype: dict[str, int] = {}
    for row in confirmed:
        by_kind[row.mutation_kind] = by_kind.get(row.mutation_kind, 0) + 1
        canon = row.kind or canonical_kind(row.mutation_kind)
        by_canonical[canon] = by_canonical.get(canon, 0) + 1
        if row.accepted_by:
            by_accepted[row.accepted_by] = by_accepted.get(row.accepted_by, 0) + 1
        label = row.hole_subtype or "(empty)"
        by_subtype[label] = by_subtype.get(label, 0) + 1

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
        if bucket in LEDGER_DISPOSITIONS:
            continue
        lines.append(f"| `{bucket}` | {buckets.get(bucket, 0)} |")
    integrity = integrity_report(mutant_rows)
    lines.extend(
        [
            f"| **Total (mutants run)** | **{mutants_run}** |",
            "",
            f"Bucket sum check: {bucket_sum} == {mutants_run} ({ 'OK' if bucket_sum == mutants_run else 'MISMATCH'}).",
            "",
            "## Bucket integrity",
            "",
            f"- `rejected-both` rows lacking `current=rejected` AND `route=rejected`: "
            f"**{integrity['rejected_both_not_dual_reject']}** "
            f"(must be 0; route `stopped` / `outside` / `no-entry` are not rejections).",
            f"- `rejected-current-route-outside` (invalid + current rejects + route outside): "
            f"**{integrity['rejected_current_route_outside_count']}**",
            f"- `valid-route-outside` (valid mutants only): "
            f"**{integrity['valid_route_outside_count']}**",
            f"- Machine-header `drop_requires` rows with both checker verdicts: "
            f"**{integrity['header_drop_requires_rows']}** "
            f"(bucket split: {integrity['header_drop_requires_by_bucket'] or 'n/a'}).",
            "",
            f"- Fixtures in shard: {meta.get('fixtures', shard_fixtures)}",
            f"- Fixtures processed: {meta.get('fixtures_processed', shard_fixtures)}",
            f"- Mutant rows: {mutants_run}",
            f"- TSV rows total (mutants + fixture ledger): {len(rows)}",
            "",
            "## Fixture ledger (1130 fixtures)",
            "",
            f"- Fixtures in shard: **{ledger['shard_fixtures']}**",
            f"- Mutated fixtures (≥1 mutant row): **{ledger['mutated']}**",
            f"- `no-mutation` ledger rows: **{ledger['no_mutation']}**",
            f"- `original-not-accepted` ledger rows: **{ledger['original_not_accepted']}**",
            f"- `failed-run` ledger rows: **{ledger['failed_run']}**",
            (
                f"- Check: {ledger['shard_fixtures']} == "
                f"{ledger['mutated']} + {ledger['no_mutation']} + "
                f"{ledger['original_not_accepted']} + {ledger['failed_run']} "
                f"({'OK' if ledger['shard_fixtures'] == ledger['mutated'] + ledger['no_mutation'] + ledger['original_not_accepted'] + ledger['failed_run'] else 'MISMATCH'})"
            ),
            "",
            f"- `drop_requires` mutant rows: **{sum(1 for row in mutant_rows if row.mutation_kind == 'drop_requires_line')}**",
            "",
            "## Fixture reconcile",
            "",
            f"- Fixtures in shard: {shard_fixtures}",
            f"- Fixtures with ≥1 mutant row: {mutated_fixtures}",
            f"- Fixtures with no mutant rows: {no_mutation}",
            f"- Mutant rows total: {mutants_run}",
            (
                f"- Avg mutants / mutated fixture: {mutants_run / mutated_fixtures:.2f}"
                if mutated_fixtures
                else "- Avg mutants / mutated fixture: n/a"
            ),
            "",
            "### Fixtures with more than one mutant row",
            "",
        ]
    )
    if multi_row:
        for fixture, count in multi_row[:50]:
            lines.append(f"- `{fixture}`: {count} rows")
        if len(multi_row) > 50:
            lines.append(f"- … and {len(multi_row) - 50} more")
    else:
        lines.append("_None yet._")
    lines.extend(
        [
            "",
            "## Mutant total accounting",
            "",
            meta.get(
                "total_note",
                "Primary pass recorded 840 mutants before `drop_requires_line` was "
                "re-enabled; backfill adds only missing `drop_requires_line` rows "
                "without deleting prior mutants.",
            ),
            "",
            "### Confirmed-hole by mutation kind",
            "",
        ]
    )
    if by_kind:
        for kind, count in sorted(by_kind.items(), key=lambda item: (-item[1], item[0])):
            lines.append(f"- `{kind}`: {count}")
    else:
        lines.append("_None._")
    lines.extend(["", "### Confirmed-hole by canonical `kind`", ""])
    if by_canonical:
        for kind, count in sorted(by_canonical.items(), key=lambda item: (-item[1], item[0])):
            lines.append(f"- `{kind}`: {count}")
    else:
        lines.append("_None._")
    lines.extend(["", "### Confirmed-hole by accepted_by", ""])
    if by_accepted:
        for who, count in sorted(by_accepted.items()):
            lines.append(f"- `{who}`: {count}")
    else:
        lines.append("_None._")
    lines.extend(["", "### Confirmed-hole by hole_subtype", ""])
    if by_subtype:
        for label, count in sorted(by_subtype.items()):
            lines.append(f"- `{label}`: {count}")
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
        ]
    )
    def hand_block(title: str, sample_rows: list[MutantRecord]) -> None:
        lines.append(title)
        lines.append("")
        if not sample_rows:
            lines.append("_None yet._")
            return
        for index, row in enumerate(sample_rows, 1):
            exit_code, out, err = capture_check_output(Path(row.mutant_path))
            route_out = capture_route_output(Path(row.mutant_path))
            lines.extend(
                [
                    f"### {title} {index}: `{row.original_fixture}` / `{row.mutation_kind}`",
                    "",
                    f"- Obligation / note: `{row.obligation_line}`",
                    f"- Route reason (hint only): `{row.route_reason}`",
                    f"- Validity note: `{row.validity_note}`",
                    f"- Accepted by: `{row.accepted_by}`",
                    f"- Disposition: `{row.disposition}` ({row.hand_check})",
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

    hand_block("## Hand verification (5 confirmed-hole)", confirmed[:5])
    disagreement = [row for row in rows if row.disposition == "disagreement-valid"][:3]
    if disagreement:
        hand_block("## Hand verification (disagreement-valid sample)", disagreement)
    drop_discarded = [
        row
        for row in rows
        if row.mutation_kind == "drop_requires_line"
        and row.disposition == "discarded-valid"
        and row.current_verdict == "accepted"
    ][:5]
    hand_block("## Hand verification (5 discarded drop_requires)", drop_discarded)
    q7 = [row for row in rows if row.disposition == "known-gap-q7-mut"]
    hand_block("## Hand verification (5 known-gap-q7-mut)", q7[:5])
    lines.extend(["## Confirmed-hole table", ""])
    if not confirmed:
        lines.append("_None._")
    else:
        lines.extend(
            [
                "| Fixture | kind | accepted_by | subtype | spec quote | minimal diff | no-trap evidence |",
                "| --- | --- | --- | --- | --- | --- | --- |",
            ]
        )
        for row in confirmed:
            quote = (row.spec_quote or "").replace("|", "\\|").replace("\n", " ")
            diff = (row.minimal_diff or "").replace("|", "\\|").replace("\n", " \\n ")
            trap = (row.no_runtime_trap_evidence or "").replace("|", "\\|")
            subtype = (row.hole_subtype or "n/a").replace("|", "\\|")
            fixture = row.original_fixture.replace("|", "\\|")
            lines.append(
                f"| `{fixture}` | `{row.kind or canonical_kind(row.mutation_kind)}` "
                f"| `{row.accepted_by}` | `{subtype}` | {quote} | `{diff[:240]}` | {trap} |"
            )
    lines.extend(["", "## Confirmed-hole entries", ""])
    if not confirmed:
        lines.append("_None._")
    else:
        for index, row in enumerate(confirmed, 1):
            original = read_text(REPO / row.original_fixture)
            mutant = read_text(Path(row.mutant_path))
            diff = minimal_mutant_diff(original, mutant)
            lines.extend(
                [
                    f"### {index}. `{row.original_fixture}` — `{row.mutation_kind}`",
                    "",
                    f"- Mutant: `{row.mutant_path}`",
                    f"- Current: **{row.current_verdict}** | Route: **{row.route_verdict}**",
                    f"- Obligation: `{row.obligation_line}`",
                    f"- Accepted by: **{row.accepted_by}**",
                    f"- Hole subtype: **{row.hole_subtype or 'n/a'}**",
                    f"- Validity note: `{row.validity_note}`",
                    f"- Route reason (hint only): `{row.route_reason}`",
                    "",
                    f"**{row.spec_path}** — *{row.spec_heading}* (check-time proof obligation):",
                    "",
                    f"> {row.spec_quote}",
                    "",
                    "**Minimal mutant diff:**",
                    "",
                    "```diff",
                    diff or "(no line diff)",
                    "```",
                    "",
                ]
            )
    lines.extend(["## Known gap: Q7 mut", ""])
    lines.append(f"Count: {len(q7)} (tracked migration; not counted as confirmed holes).")
    MD_PATH.write_text("\n".join(lines) + "\n", encoding="utf-8")
    write_stats(rows, meta)


def process_fixture_mutations(
    fixture: str, mutations: list[tuple[str, str]], source: str
) -> list[MutantRecord]:
    main_src = REPO / fixture
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
    if not batch_meta:
        return []

    current_by_path: dict[str, str] = {}
    with ThreadPoolExecutor(max_workers=4) as pool:
        futures = {
            pool.submit(run_omega_check, main_path): (kind, main_path)
            for kind, main_path in batch_meta
        }
        for future in as_completed(futures):
            _kind, main_path = futures[future]
            verdict, _seconds = future.result()
            current_by_path[str(main_path)] = verdict

    route_results = run_route(batch_dirs)
    fixture_rows: list[MutantRecord] = []
    for kind, main_path in batch_meta:
        key = str(main_path)
        current = current_by_path.get(key, "unknown")
        route, _, rsec, route_reason = route_results.get(
            key, ("unknown", "unknown", 0.0, "")
        )
        mutant = read_text(main_path)
        result = classify_mutant(
            kind, current, route, source, mutant, route_reason=route_reason
        )
        spec_path = spec_quote = spec_heading = ""
        if result.spec:
            spec_path = result.spec.path
            spec_quote = result.spec.quote
            spec_heading = result.spec.heading
        hole_subtype = result.hole_subtype
        if (
            kind == "drop_requires_line"
            and result.disposition == "confirmed-hole"
            and result.accepted_by
        ):
            removed = drop_requires_removed_line(source, mutant)
            hole_subtype = probe_drop_requires_subtype(main_path, mutant, removed)
        fixture_rows.append(
            build_record(
                fixture=fixture,
                kind=kind,
                mutant_path=key,
                current=current,
                route=route,
                result=result,
                source=source,
                mutant=mutant,
                route_reason=route_reason,
                hole_subtype=hole_subtype,
                seconds=rsec,
            )
        )
    return fixture_rows


def backfill_drop_requires() -> int:
    done = load_progress()
    existing_paths: set[str] = set()
    if TSV_PATH.exists():
        for line in TSV_PATH.read_text().splitlines()[1:]:
            old = parse_tsv_line(line.split("\t"))
            if (
                old
                and is_mutant_row(old)
                and old.mutation_kind == "drop_requires_line"
            ):
                existing_paths.add(old.mutant_path)
    added = 0
    variants_added = 0
    for fixture in sorted(done):
        source = read_text(REPO / fixture)
        pending = drop_requires_backfill_pending(fixture, source, existing_paths)
        if not pending:
            continue
        rows = process_fixture_mutations(fixture, pending, source)
        append_tsv(rows)
        added += len(rows)
        variants_added += len(pending)
    all_rows = revalidate_tsv_rows()
    meta = json.loads(PROGRESS_PATH.read_text()) if PROGRESS_PATH.exists() else {}
    counts = meta.get("counts", {})
    counts["mutants_generated"] = len(all_rows)
    drop_rows = sum(
        1 for row in all_rows if is_mutant_row(row) and row.mutation_kind == "drop_requires_line"
    )
    counts["total_note"] = (
        f"Backfill added {added} `drop_requires_line` row(s) "
        f"({variants_added} new variant(s)); total drop_requires rows: {drop_rows}."
    )
    host = subprocess.check_output(["hostname"], text=True).strip()
    write_md(all_rows, counts, host)
    print(
        json.dumps(
            {
                "backfill_added": added,
                "drop_requires_rows": drop_rows,
                "tsv_rows": len(all_rows),
            },
            indent=2,
        )
    )
    return 0


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
        mutant_rows = [row for row in rows if is_mutant_row(row)]
        print(
            json.dumps(
                {
                    "revalidated": len(rows),
                    "mutant_rows": len(mutant_rows),
                    "buckets": bucket_counts_from_rows(rows),
                    "integrity": integrity_report(mutant_rows),
                },
                indent=2,
            )
        )
        return 0

    if "--backfill-drop-requires" in sys.argv:
        return backfill_drop_requires()

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
        source = read_text(REPO / fixture)
        mutations = generate_mutations(source)
        fixture_rows = process_fixture_mutations(fixture, mutations, source)
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
