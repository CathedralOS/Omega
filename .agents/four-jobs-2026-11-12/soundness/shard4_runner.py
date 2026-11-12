#!/usr/bin/env python3
"""Job 2 soundness mutations — shard 4 runner (measurement only)."""

from __future__ import annotations

import hashlib
import os
import re
import shutil
import subprocess
import sys
import time
from collections import Counter
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 4
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/soundness"
MUT_ROOT = OUT_DIR / f"shard-{SHARD}"
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
PROGRESS = OUT_DIR / f"shard-{SHARD}.progress"
STATS = OUT_DIR / f"shard-{SHARD}.stats.json"
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
CHECK_TIMEOUT = 90
WORKERS = 4


@dataclass
class Verdicts:
    current: str
    route: str
    seconds: float


def list_fixtures() -> list[Path]:
    proc = subprocess.run(
        "git ls-files 'tests/omega/pass/**/main.omg' 'tests/omega/run/**/main.omg' | LC_ALL=C sort",
        cwd=REPO,
        capture_output=True,
        text=True,
        check=True,
        shell=True,
    )
    lines = sorted(proc.stdout.splitlines(), key=str.lower)
    out: list[Path] = []
    for i, rel in enumerate(lines):
        if i % 8 == SHARD:
            out.append(REPO / rel)
    return out


def fixture_tier(path: Path) -> str:
    rel = path.relative_to(REPO / "tests" / "omega")
    return rel.parts[0]


def run_checker(cmd: list[str]) -> tuple[int, str]:
    env = os.environ.copy()
    env["OMEGA_LIBRARY_ROOT"] = str(REPO / "source/library")
    try:
        proc = subprocess.run(
            cmd,
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
            timeout=CHECK_TIMEOUT,
            stdin=subprocess.DEVNULL,
        )
        return proc.returncode, proc.stdout + proc.stderr
    except subprocess.TimeoutExpired:
        return -1, "TIMEOUT"


def parse_current(output: str, code: int) -> str:
    if code == -1:
        return "timeout"
    if code == 0:
        return "accepted"
    return "rejected"


def parse_route(output: str, code: int) -> str:
    if code == -1:
        return "timeout"
    for line in output.splitlines():
        if line.startswith("program\t"):
            m = re.search(r"\broute=([^\s]+)", line)
            if m:
                return m.group(1)
            parts = line.split("\t")
            for p in parts:
                if p.startswith("route="):
                    return p.split("=", 1)[1].split()[0]
    return "error"


def parse_route_reason(output: str, code: int) -> str:
    if code == -1:
        return "timeout"
    for line in output.splitlines():
        if not line.startswith("program\t"):
            continue
        m = re.search(r"\broute=(\S+)\s*(.*)", line)
        if m:
            tail = m.group(2).strip()
            if tail:
                return one_line(tail)
            verdict = m.group(1)
            if verdict in ("rejected", "fallback", "incomplete"):
                break
    for line in output.splitlines():
        if line.startswith("diagnostic\t") or line.startswith("fallback\t"):
            parts = line.split("\t")
            if len(parts) >= 6:
                return one_line(parts[-1])
    for line in output.splitlines():
        if line.startswith("program\t"):
            m = re.search(r"\broute=(\S+)", line)
            if m:
                return m.group(1)
    return ""


def one_line(text: str) -> str:
    return re.sub(r"[\t\n]+", " ", text)[:400]


def check_pair(main_omg: Path) -> Verdicts:
    t0 = time.monotonic()
    c_code, c_out = run_checker(
        ["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(main_omg)]
    )
    current = parse_current(c_out, c_code)
    r_code, r_out = run_checker(
        ["timeout", str(CHECK_TIMEOUT), str(ROUTE), str(main_omg)]
    )
    route = parse_route(r_out, r_code)
    return Verdicts(current, route, time.monotonic() - t0)


def accepts(v: str) -> bool:
    return v == "accepted"


JUDICIAL_ROUTE = frozenset({"accepted", "rejected", "incomplete", "fallback", "defect"})


def judicial_route(route: str) -> bool:
    return route in JUDICIAL_ROUTE


def disagrees(cur: str, route: str) -> bool:
    if cur in ("timeout", "error") or route in ("timeout", "error"):
        return False
    if not judicial_route(route):
        return False
    ca, ra = accepts(cur), accepts(route)
    return ca != ra


def slug(s: str) -> str:
    h = hashlib.sha256(s.encode()).hexdigest()[:10]
    safe = re.sub(r"[^a-zA-Z0-9_-]+", "_", s)[:80]
    return f"{safe}_{h}"


def copy_fixture(src_main: Path, dst_root: Path) -> None:
    src_dir = src_main.parent
    if dst_root.exists():
        shutil.rmtree(dst_root)
    shutil.copytree(src_dir, dst_root)


def apply_mutation(text: str, kind: str) -> str | None:
    if kind == "drop_requires":
        if "requires" not in text:
            return None
        out = re.sub(r"^\s*requires\b[^\n]*\n", "", text, flags=re.MULTILINE)
        return out if out != text else None
    if kind == "widen_requires_lt":
        if not re.search(r"requires\b[^\n]*<", text):
            return None
        return re.sub(r"(requires\b[^\n]*)(\d+)(\s*<\s*)(\w)", r"\1\2\3\2", text, count=1)
    if kind == "remove_transition_guard":
        m = re.search(r"transition\s+([^{;\n]+)\s*\{", text)
        if not m:
            return None
        cond = m.group(1).strip()
        if cond in ("{", "") or cond.startswith("_"):
            return None
        return text.replace(f"transition {cond} {{", "transition {", 1)
    if kind == "strip_and_guard":
        if "&&" not in text:
            return None
        return re.sub(r"(\btransition\b[^\n]*)\s*&&\s*[^|\n{]+", r"\1", text, count=1)
    if kind == "ref_to_mut_param":
        if "&mut " not in text:
            return None
        return text.replace("&mut ", "&", 1)
    if kind == "mut_to_ref_param":
        if re.search(r":\s*&\w", text) and "&mut " not in text:
            return text.replace(": &", ": &mut ", 1)
        return None
    if kind == "drop_mut_let":
        if "let mut " not in text:
            return None
        return text.replace("let mut ", "let ", 1)
    if kind == "add_mut_let":
        m = re.search(r"\blet\s+(\w+)", text)
        if not m or "let mut " in text[: m.start() + 20]:
            return None
        return text.replace("let ", "let mut ", 1)
    if kind == "delete_divisor_guard":
        if "/" not in text:
            return None
        # Remove a conjunct guarding division (heuristic: drop `&& x != 0` near `/`)
        return re.sub(r"\s*&&\s*\w+\s*!=\s*0", "", text, count=1)
    return None


MUTATION_KINDS = [
    "drop_requires",
    "widen_requires_lt",
    "remove_transition_guard",
    "strip_and_guard",
    "ref_to_mut_param",
    "drop_mut_let",
    "delete_divisor_guard",
]


def mutant_dir(fixture: Path, kind: str) -> Path:
    rel = fixture.relative_to(REPO).as_posix()
    return MUT_ROOT / slug(f"{rel}_{kind}")


class Progress:
    def __init__(self) -> None:
        self.done: set[str] = set()
        if PROGRESS.exists():
            self.done = {ln.strip() for ln in PROGRESS.read_text().splitlines() if ln.strip()}

    def seen(self, key: str) -> bool:
        return key in self.done

    def mark(self, key: str) -> None:
        with _IO_LOCK:
            if key in self.done:
                return
            self.done.add(key)
            with PROGRESS.open("a") as f:
                f.write(key + "\n")


_PROGRESS: Progress | None = None
_IO_LOCK = __import__("threading").Lock()


def progress() -> Progress:
    global _PROGRESS
    if _PROGRESS is None:
        _PROGRESS = Progress()
    return _PROGRESS


def ensure_tsv_header() -> None:
    if not TSV.exists():
        TSV.write_text(
            "fixture\ttier\tstage\tmutation_kind\tmutant_path\tcurrent_verdict\t"
            "route_verdict\troute_rejection_reason\tseconds\thand_check\t"
            "spec_rule_file\tspec_quote\tbucket\tobligation_line\n"
        )


STATS_COUNTERS: Counter = Counter()


def bump_stat(key: str, n: int = 1) -> None:
    with _IO_LOCK:
        STATS_COUNTERS[key] += n


def flush_stats() -> None:
    with _IO_LOCK:
        STATS.write_text(json.dumps(dict(STATS_COUNTERS), indent=2) + "\n")


def log_tsv(row: list[str]) -> None:
    with _IO_LOCK:
        with TSV.open("a") as f:
            f.write("\t".join(row) + "\n")


# Spec quotes for confirmed-invalid by mutation kind (filled during reconcile review pass)
SPEC_HINTS: dict[str, tuple[str, str]] = {}


def hand_check(kind: str, text: str, cur: str, route: str) -> tuple[str, str, str]:
    """Returns hand_check, spec_file, spec_quote (empty if discarded)."""
    if not disagrees(cur, route) and not (
        judicial_route(route) and (accepts(cur) or accepts(route))
    ):
        return "discarded-both-reject", "", ""
    if disagrees(cur, route):
        return "confirmed-disagreement", "", ""
    if judicial_route(route) and (accepts(cur) or accepts(route)):
        return "pending-spec", "", ""
    return "discarded-valid", "", ""


def check_current_only(main_omg: Path) -> Verdicts:
    t0 = time.monotonic()
    c_code, c_out = run_checker(
        ["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(main_omg)]
    )
    return Verdicts(parse_current(c_out, c_code), "", time.monotonic() - t0)


def process_fixture(fixture: Path) -> int:
    rel = fixture.relative_to(REPO).as_posix()
    key = f"fixture:{rel}"
    prog = progress()
    if prog.seen(key):
        return 0
    tier = fixture_tier(fixture)
    v = check_current_only(fixture)
    log_tsv(
        [
            rel,
            tier,
            "original",
            "",
            "",
            v.current,
            "",
            "",
            f"{v.seconds:.2f}",
            "baseline",
            "",
            "",
            "",
            "",
        ]
    )
    if not accepts(v.current):
        prog.mark(key)
        return 0
    kept = 0
    text = fixture.read_text()
    kinds_possible = list(MUTATION_KINDS)
    for kind in kinds_possible:
        mk = f"{key}:{kind}"
        if prog.seen(mk):
            continue
        mutated = apply_mutation(text, kind)
        if mutated is None:
            bump_stat("mutants_not_applicable")
            prog.mark(mk)
            continue
        bump_stat("mutants_generated")
        dst = mutant_dir(fixture, kind)
        copy_fixture(fixture, dst)
        (dst / "main.omg").write_text(mutated)
        c_code, c_out = run_checker(
            ["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(dst / "main.omg")]
        )
        cur_v = parse_current(c_out, c_code)
        bump_stat("mutants_run")
        route_reason = ""
        if cur_v == "rejected":
            bump_stat("mutants_current_rejected")
            bump_stat("mutants_logged_tsv")
            log_tsv(
                [
                    rel,
                    tier,
                    "mutant",
                    kind,
                    str(dst.relative_to(REPO)),
                    cur_v,
                    "",
                    "",
                    "0",
                    "discarded-both-reject",
                    "",
                    "",
                    "discarded-both-reject",
                    "",
                ]
            )
            prog.mark(mk)
            continue
        if cur_v in ("timeout", "error"):
            bump_stat("mutants_failed_run")
            prog.mark(mk)
            continue
        t0 = time.monotonic()
        r_code, r_out = run_checker(
            ["timeout", str(CHECK_TIMEOUT), str(ROUTE), str(dst / "main.omg")]
        )
        route_v = parse_route(r_out, r_code)
        route_reason = parse_route_reason(r_out, r_code)
        mv = Verdicts(cur_v, route_v, time.monotonic() - t0)
        hand, sfile, squote = hand_check(kind, mutated, mv.current, mv.route)
        log_drop_requires = kind == "drop_requires" and accepts(cur_v)
        interesting = log_drop_requires or accepts(mv.current) or (
            judicial_route(mv.route) and disagrees(mv.current, mv.route)
        )
        if interesting:
            bump_stat("mutants_logged_tsv")
            log_tsv(
                [
                    rel,
                    tier,
                    "mutant",
                    kind,
                    str(dst.relative_to(REPO)),
                    mv.current,
                    mv.route,
                    route_reason,
                    f"{mv.seconds:.2f}",
                    hand,
                    sfile,
                    squote,
                    "",
                    "",
                ]
            )
            kept += 1
        prog.mark(mk)
    prog.mark(key)
    return kept


def applicable_kinds(text: str) -> list[str]:
    kinds: list[str] = []
    for kind in MUTATION_KINDS:
        if apply_mutation(text, kind) is not None:
            kinds.append(kind)
    return kinds


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    MUT_ROOT.mkdir(parents=True, exist_ok=True)
    ensure_tsv_header()
    fixtures = list_fixtures()
    limit = int(os.environ.get("SHARD_LIMIT", "0"))
    if limit > 0:
        fixtures = fixtures[:limit]
    print(f"Shard {SHARD}: {len(fixtures)} fixtures", flush=True)
    total_kept = 0
    done_count = 0

    def run_one(i: int, fx: Path) -> int:
        nonlocal done_count
        k = process_fixture(fx)
        with _IO_LOCK:
            done_count += 1
            if done_count % 50 == 0:
                print(f"progress {done_count}/{len(fixtures)} kept={total_kept}", flush=True)
        return k

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        futs = [pool.submit(run_one, i, fx) for i, fx in enumerate(fixtures)]
        for fut in as_completed(futs):
            try:
                total_kept += fut.result()
            except Exception as exc:
                print(f"fixture error: {exc}", flush=True)
    flush_stats()
    print(f"Done. candidates_logged={total_kept}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
