#!/usr/bin/env python3
"""Soundness mutation shard runner (job 2). Read-only on repo fixtures."""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, asdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 2
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
MUT_ROOT = OUT / f"shard-{SHARD}"
TSV = OUT / f"shard-{SHARD}.tsv"
PROGRESS = OUT / f"shard-{SHARD}.progress.json"
CHECK_TIMEOUT = 120
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"

ENV = {
    **os.environ,
    "OMEGA_LIBRARY_ROOT": str(REPO / "source/library"),
    "RUST_BACKTRACE": "0",
}


def fixture_list() -> list[str]:
    proc = subprocess.run(
        [
            "git",
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
        ],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=True,
    )
    lines = sorted(proc.stdout.splitlines())
    return [p for i, p in enumerate(lines) if i % 8 == SHARD]


def tier(path: str) -> str:
    return "pass" if path.startswith("tests/omega/pass/") else "run"


def run_cmd(cmd: list[str], cwd: Path | None = None) -> tuple[str, int, float]:
    t0 = time.monotonic()
    try:
        r = subprocess.run(
            cmd,
            cwd=cwd or REPO,
            capture_output=True,
            text=True,
            timeout=CHECK_TIMEOUT,
            env=ENV,
            stdin=subprocess.DEVNULL,
        )
        out = (r.stdout or "") + (r.stderr or "")
        return out, r.returncode, time.monotonic() - t0
    except subprocess.TimeoutExpired as e:
        partial = (e.stdout or b"").decode(errors="replace") + (
            e.stderr or b""
        ).decode(errors="replace")
        return partial + "\n[TIMEOUT]\n", 124, time.monotonic() - t0


def route_rejection_detail(out: str, route: str) -> str:
    parts: list[str] = []
    for line in out.splitlines():
        if line.startswith("program") and "root=compilation" in line:
            tail = line.split(f"route={route}", 1)
            if len(tail) > 1 and tail[1].strip():
                parts.append(tail[1].strip().lstrip("\t"))
        elif line.startswith("diagnostic\t") and "root=compilation" in line:
            msg = line.split("\t", 4)[-1] if line.count("\t") >= 4 else line
            parts.append(one_line_diag(msg))
        elif line.startswith("fallback\t"):
            parts.append(line.split("\t", 3)[-1] if "\t" in line else line)
    return "; ".join(parts)[:500]


def one_line_diag(text: str) -> str:
    return text.replace("\t", " ").replace("\n", " ")[:240]


def check_both(path: Path) -> tuple[str, str, str, str]:
    """Current, route, route rejection detail, raw output (truncated)."""
    out, code, _ = run_cmd([str(ROUTE), str(path)])
    if code == 124:
        return "timeout", "timeout", "timeout", out[:8000]
    current = route = "unknown"
    for line in out.splitlines():
        if not line.startswith("program"):
            continue
        if "root=compilation" in line or "root=" not in line:
            mc = re.search(r"[\s\t]current=(\S+)", line)
            mr = re.search(r"[\s\t]route=(\S+)", line)
            if mc:
                current = mc.group(1)
            if mr:
                route = mr.group(1)
    if current == "unknown" and code == 0:
        current = "accepted"
    if route == "unknown" and code != 0:
        route = "error"
    detail = route_rejection_detail(out, route) if route not in ("accepted", "unknown") else ""
    return current, route, detail, out[:12000]


def current_verdict(path: Path) -> str:
    cv, _, _, _ = check_both(path)
    return cv


def parse_route_only(raw: str) -> str:
    for line in raw.splitlines():
        if line.startswith("program"):
            mr = re.search(r"[\s\t]route=(\S+)", line)
            if mr:
                return mr.group(1)
    return "unknown"


def copy_fixture_package(fixture_rel: str, dest: Path) -> None:
    """Copy full fixture package (build.omg, targets/, siblings) for ProgramEntry bindings."""
    src_dir = (REPO / fixture_rel).parent
    dest.mkdir(parents=True, exist_ok=True)
    for name in src_dir.iterdir():
        if name.name == "main.omg":
            continue
        if name.is_file():
            shutil.copy2(name, dest / name.name)
        elif name.is_dir():
            shutil.copytree(name, dest / name.name, dirs_exist_ok=True)


@dataclass
class Mutation:
    kind: str
    rule_hint: str  # wiki/spec path for hand review


MUTATORS: list[tuple[str, str, callable]] = []


def mutator(kind: str, rule_hint: str):
    def deco(fn):
        MUTATORS.append((kind, rule_hint, fn))
        return fn

    return deco


@mutator("drop_machine_requires", "wiki/spec/language/state_contracts.md")
def drop_machine_requires(text: str) -> str | None:
    if not re.search(r"machine[^\n]*\n\s*requires\b", text):
        return None
    out = re.sub(
        r"(machine[^\n]*\n)\s*requires\b[^\n]*\n",
        r"\1",
        text,
        count=1,
    )
    return out if out != text else None


@mutator("drop_machine_ensures", "wiki/spec/language/state_contracts.md")
def drop_machine_ensures(text: str) -> str | None:
    out = re.sub(
        r"(machine[^\n]*\n)\s*ensures\b[^\n]*\n",
        r"\1",
        text,
        count=1,
    )
    return out if out and out != text else None


@mutator("widen_requires_lt", "wiki/spec/language/state_contracts.md")
def widen_requires_lt(text: str) -> str | None:
    lines = text.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if line.strip().startswith("requires") and "<" in line and "<=" not in line:
            new = line.replace("<", "<=", 1)
            if new != line:
                lines[i] = new
                return "".join(lines)
    return None


@mutator("remove_mut", "wiki/spec/language/ownership.md")
def remove_mut(text: str) -> str | None:
    out = re.sub(r"\blet mut\b", "let", text, count=1)
    return out if out != text else None


@mutator("ref_to_mut_ref", "wiki/spec/language/ownership.md")
def ref_to_mut_ref(text: str) -> str | None:
    out = re.sub(r"(?<![&])&(?![&])", "&mut ", text, count=1)
    out = out.replace("&mut mut ", "&mut ")
    return out if out != text else None


@mutator("mut_ref_to_ref", "wiki/spec/language/ownership.md")
def mut_ref_to_ref(text: str) -> str | None:
    out = re.sub(r"&mut\s+", "&", text, count=1)
    return out if out != text else None


@mutator("delete_first_if_guard", "wiki/spec/language/guards.md")
def delete_first_if_guard(text: str) -> str | None:
    pat = re.compile(
        r"(?ms)^(\s*)if\s+[^\{]+\{\s*\n(.*?)^\1\}\s*else\s*\{\s*\n(.*?)^\1\}\s*\n",
        re.MULTILINE,
    )
    m = pat.search(text)
    if not m:
        pat2 = re.compile(r"(?ms)^(\s*)if\s+[^\{]+\{\s*\n(.*?)^\1\}\s*\n", re.MULTILINE)
        m = pat2.search(text)
        if not m:
            return None
        body = m.group(2)
        return text[: m.start()] + body + text[m.end() :]
    body = m.group(3)
    return text[: m.start()] + body + text[m.end() :]


def mutant_id(fixture_rel: str, kind: str, idx: int) -> str:
    h = hashlib.sha256(f"{fixture_rel}:{kind}:{idx}".encode()).hexdigest()[:12]
    slug = fixture_rel.replace("/", "_").replace("main.omg", "")[:80]
    return f"{slug}{kind}_{h}"


def load_progress() -> dict:
    if PROGRESS.exists():
        return json.loads(PROGRESS.read_text())
    return {"fixtures_done": [], "stats": {}}


def save_progress(prog: dict) -> None:
    PROGRESS.parent.mkdir(parents=True, exist_ok=True)
    PROGRESS.write_text(json.dumps(prog, indent=2))


def append_tsv(row: dict) -> None:
    cols = [
        "fixture",
        "tier",
        "mutation_kind",
        "mutant_path",
        "current_verdict",
        "route_verdict",
        "run_disposition",
        "route_rejection_detail",
        "rule_hint",
        "seconds",
    ]
    exists = TSV.exists()
    with TSV.open("a") as f:
        if not exists:
            f.write("\t".join(cols) + "\n")
        f.write("\t".join(str(row.get(c, "")) for c in cols) + "\n")


def main() -> int:
    if not OMEGA.is_file() or not ROUTE.is_file():
        print("Build omega and route_one_comparison first", file=sys.stderr)
        return 1

    fixtures = fixture_list()
    prog = load_progress()
    done = set(prog.get("fixtures_done", []))
    stats = prog.get(
        "stats",
        {
            "fixtures_total": len(fixtures),
            "fixtures_skipped_original_fails": 0,
            "mutants_generated": 0,
            "mutants_run": 0,
            "mutator_skipped_no_apply": 0,
            "mutant_run_failed": 0,
            "current_accepts": 0,
            "both_rejected": 0,
        },
    )

    MUT_ROOT.mkdir(parents=True, exist_ok=True)

    for fixture_rel in fixtures:
        if fixture_rel in done:
            continue
        src_main = REPO / fixture_rel
        orig_path = src_main
        orig_v = current_verdict(orig_path)
        if orig_v != "accepted":
            stats["fixtures_skipped_original_fails"] = (
                stats.get("fixtures_skipped_original_fails", 0) + 1
            )
            skip_log = stats.setdefault("original_skip_log", [])
            _, _, detail, raw = check_both(orig_path)
            skip_log.append(
                {
                    "fixture": fixture_rel,
                    "current_verdict": orig_v,
                    "route_verdict": parse_route_only(raw),
                    "diagnostic": detail or raw[:500],
                }
            )
            done.add(fixture_rel)
            prog["fixtures_done"] = list(done)
            prog["stats"] = stats
            save_progress(prog)
            continue

        text = src_main.read_text()
        mutants_this_fixture = 0
        for kind, rule_hint, fn in MUTATORS:
            mutated = fn(text)
            if mutated is None:
                stats["mutator_skipped_no_apply"] = (
                    stats.get("mutator_skipped_no_apply", 0) + 1
                )
                continue
            stats["mutants_generated"] = stats.get("mutants_generated", 0) + 1
            mutants_this_fixture += 1
            mid = mutant_id(fixture_rel, kind, 0)
            mdir = MUT_ROOT / mid
            copy_fixture_package(fixture_rel, mdir)
            (mdir / "main.omg").write_text(mutated)
            mpath = mdir / "main.omg"

            t0 = time.monotonic()
            try:
                cv, rv, route_detail, _raw = check_both(mpath)
            except Exception as exc:
                stats["mutant_run_failed"] = stats.get("mutant_run_failed", 0) + 1
                append_tsv(
                    {
                        "fixture": fixture_rel,
                        "tier": tier(fixture_rel),
                        "mutation_kind": kind,
                        "mutant_path": "",
                        "current_verdict": "error",
                        "route_verdict": "error",
                        "run_disposition": "run_failed",
                        "route_rejection_detail": one_line_diag(str(exc)),
                        "rule_hint": rule_hint,
                        "seconds": f"{time.monotonic() - t0:.2f}",
                    }
                )
                shutil.rmtree(mdir, ignore_errors=True)
                continue
            secs = time.monotonic() - t0
            stats["mutants_run"] = stats.get("mutants_run", 0) + 1

            if cv == "timeout" or rv == "timeout":
                run_disposition = "timeout"
                shutil.rmtree(mdir, ignore_errors=True)
            elif cv in ("rejected", "stopped") and rv in (
                "rejected",
                "incomplete",
                "defect",
                "outside",
                "fallback",
                "no-entry",
                "unreached",
            ):
                stats["both_rejected"] = stats.get("both_rejected", 0) + 1
                run_disposition = "rejected_both"
                shutil.rmtree(mdir, ignore_errors=True)
            elif cv == "accepted" or rv == "accepted":
                stats["checker_accepts"] = stats.get("checker_accepts", 0) + 1
                if cv == "accepted":
                    stats["current_accepts"] = stats.get("current_accepts", 0) + 1
                if rv == "accepted":
                    stats["route_accepts"] = stats.get("route_accepts", 0) + 1
                run_disposition = "checker_accepts"
            else:
                run_disposition = "neither_accepts"
                shutil.rmtree(mdir, ignore_errors=True)

            append_tsv(
                {
                    "fixture": fixture_rel,
                    "tier": tier(fixture_rel),
                    "mutation_kind": kind,
                    "mutant_path": (
                        str(mpath.relative_to(REPO))
                        if run_disposition == "checker_accepts"
                        else ""
                    ),
                    "current_verdict": cv,
                    "route_verdict": rv,
                    "run_disposition": run_disposition,
                    "route_rejection_detail": route_detail,
                    "rule_hint": rule_hint,
                    "seconds": f"{secs:.2f}",
                }
            )

        if mutants_this_fixture == 0:
            stats["fixtures_no_mutation"] = stats.get("fixtures_no_mutation", 0) + 1
            prog.setdefault("fixtures_no_mutation", []).append(fixture_rel)
        else:
            stats["fixtures_mutated"] = stats.get("fixtures_mutated", 0) + 1

        done.add(fixture_rel)
        prog["fixtures_done"] = list(done)
        prog["stats"] = stats
        save_progress(prog)
        if len(done) % 10 == 0:
            print(
                f"progress {len(done)}/{len(fixtures)} "
                f"checker_accepts={stats.get('checker_accepts',0)}"
            )

    print(json.dumps(stats, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
