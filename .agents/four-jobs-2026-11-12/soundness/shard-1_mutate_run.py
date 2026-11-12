#!/usr/bin/env python3
"""Shard-1 soundness mutation runner (job 2). No compiler edits."""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 1
SHARD_MOD = 8
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/soundness/shard-1"
STATE_PATH = OUT_DIR / "run_state.json"
LOG_TSV = OUT_DIR / "mutants_log.tsv"
TIMEOUT = 120
ROUTE_BIN = REPO / "target/debug/examples/route_one_comparison"
OMEGA_BIN = REPO / "target/debug/omega"


def shard_fixtures() -> list[str]:
    proc = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "tests/omega/pass/**/main.omg", "tests/omega/run/**/main.omg"],
        capture_output=True,
        text=True,
        check=True,
    )
    lines = sorted(proc.stdout.splitlines(), key=lambda s: s)
    out = []
    for i, path in enumerate(lines):
        if i % SHARD_MOD == SHARD:
            out.append(path)
    return out


def applicable_kinds(src: str) -> list[str]:
    kinds: list[str] = []
    if "requires" in src:
        kinds.extend(["drop_requires", "weaken_requires_ge", "drop_requires_and", "widen_u64_bound"])
    if "let mut" in src:
        kinds.append("let_mut_to_let")
    if "&mut" in src:
        kinds.append("ammut_to_imm")
    if re.search(r"(?<![&])&(?!&)", src):
        kinds.append("imm_to_mut")
    if "transition" in src:
        kinds.append("transition_always_true")
    if re.search(r"(?m)^\s*if\s+", src):
        kinds.append("remove_if_guard_line")
    return kinds


def mutate_source(src: str, kind: str) -> str | None:
    if kind == "drop_requires":
        out, n = re.subn(r"(?m)^requires[^\n]*\n", "", src, count=1)
        return out if n else None
    if kind == "weaken_requires_ge":
        if ">=" not in src and "requires" not in src:
            return None
        out = re.sub(r"(requires[^;{]*?)>=", r"\1>", src, count=1)
        return out if out != src else None
    if kind == "drop_requires_and":
        m = re.search(r"(?m)^(requires.*?)(&&.*?)(\n)", src)
        if not m:
            return None
        return src[: m.start(2)] + src[m.end(2) :]
    if kind == "let_mut_to_let":
        out, n = re.subn(r"\blet mut\b", "let", src, count=1)
        return out if n else None
    if kind == "ammut_to_imm":
        out, n = re.subn(r"&mut\b", "&", src, count=1)
        return out if n else None
    if kind == "imm_to_mut":
        out, n = re.subn(r"(?<![&])&(?!\s*mut\b)", "&mut", src, count=1)
        # avoid && and already-mut
        if n and "&&" in out:
            return None
        return out if n else None
    if kind == "transition_always_true":
        m = re.search(
            r"(transition\s+)([^\{]+?)(\s*\{)", src
        )
        if not m:
            return None
        return src[: m.start(2)] + "true" + src[m.start(3) :]
    if kind == "remove_if_guard_line":
        out, n = re.subn(r"(?m)^\s*if\s+[^\n\{]+\{\s*\n", "", src, count=1)
        return out if n else None
    if kind == "widen_u64_bound":
        # replace small literal in requires with 0
        m = re.search(r"(?m)^(requires.*)", src)
        if not m:
            return None
        line = m.group(1)
        new_line = re.sub(r"\b[1-9]\d*\b", "0", line, count=1)
        return src.replace(line, new_line, 1) if new_line != line else None
    return None


def write_synthetic_build(mdir: Path, app_name: str) -> None:
    text = f"""machine build(builder: &mut Build) {{
    Build::application(builder, "{app_name}");
    Build::bind(builder, macos_arm64::ProgramEntry, main);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
}}
"""
    (mdir / "build.omg").write_text(text)


def ensure_build_omg(mdir: Path, fix_dir: Path, fixture: str) -> None:
    if (mdir / "build.omg").is_file():
        return
    probe = fix_dir
    root_tests = REPO / "tests" / "omega"
    while True:
        candidate = probe / "build.omg"
        if candidate.is_file():
            shutil.copy2(candidate, mdir / "build.omg")
            return
        if probe == root_tests:
            break
        parent = probe.parent
        if parent == probe:
            break
        probe = parent
    if re.search(r"(?m)^machine main\(", (mdir / "main.omg").read_text()):
        app = Path(fixture).parent.name
        write_synthetic_build(mdir, app)


def mutant_id(fixture: str, kind: str, idx: int) -> str:
    h = hashlib.sha256(f"{fixture}:{kind}:{idx}".encode()).hexdigest()[:12]
    safe = fixture.replace("/", "__").replace(".omg", "")
    return f"{safe}__{kind}__{h}"


def parse_route_program_line(text: str) -> dict[str, str]:
    for line in text.splitlines():
        if line.startswith("program"):
            parts = {}
            for tok in re.split(r"[\s\t]+", line):
                if "=" in tok:
                    k, v = tok.split("=", 1)
                    parts[k] = v
            parts["raw"] = line
            return parts
    return {}


def run_check(cmd: list[str], cwd: Path | None = None) -> tuple[int, str, float]:
    t0 = time.monotonic()
    try:
        p = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
            stdin=subprocess.DEVNULL,
        )
        out = (p.stdout or "") + (p.stderr or "")
        return p.returncode, out, time.monotonic() - t0
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or "") + (e.stderr or "")
        return -124, out + "\nTIMEOUT", time.monotonic() - t0


def current_verdict(exit_code: int, output: str) -> str:
    if exit_code == -124:
        return "timeout"
    if exit_code != 0:
        return "rejected"
    low = output.lower()
    if "error" in low or "rejected" in low:
        return "rejected"
    return "accepted"


def route_verdict(output: str) -> str:
    info = parse_route_program_line(output)
    r = info.get("route", "missing")
    if r in ("accepted", "rejected", "incomplete", "fallback", "defect", "no-entry", "unreached"):
        if r == "accepted":
            return "accepted"
        if r in ("rejected", "incomplete"):
            return "rejected"
        if r == "fallback":
            return "outside"
        return r
    if "TIMEOUT" in output:
        return "timeout"
    return "error"


@dataclass
class MutantRecord:
    fixture: str
    kind: str
    mutant_dir: str
    current: str
    route: str
    disagree: bool
    either_accepts: bool
    seconds: float


def load_state() -> dict:
    if STATE_PATH.exists():
        return json.loads(STATE_PATH.read_text())
    return {"done_fixtures": [], "counts": {"generated": 0, "run": 0, "kept_candidate": 0}}


def save_state(state: dict) -> None:
    STATE_PATH.write_text(json.dumps(state, indent=2) + "\n")


def ensure_log_header() -> None:
    if not LOG_TSV.exists():
        LOG_TSV.write_text(
            "fixture\tkind\tmutant_dir\tcurrent\troute\tdisagree\teither_accepts\tseconds\n"
        )


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if not OMEGA_BIN.is_file() or not ROUTE_BIN.is_file():
        print("Build missing: omega or route_one_comparison", file=sys.stderr)
        return 2

    fixtures = shard_fixtures()
    state = load_state()
    done = set(state["done_fixtures"])
    ensure_log_header()

    candidates: list[MutantRecord] = []

    for fi, fixture in enumerate(fixtures):
        if fixture in done:
            continue
        fix_path = REPO / fixture
        src = fix_path.read_text()
        fix_dir = fix_path.parent

        ec0, omega0, _ = run_check([str(OMEGA_BIN), "--check", str(fix_path)])
        if current_verdict(ec0, omega0) != "accepted":
            done.add(fixture)
            state["done_fixtures"] = sorted(done)
            save_state(state)
            continue

        for kind in applicable_kinds(src):
            mutated = mutate_source(src, kind)
            if mutated is None or mutated == src:
                continue
            state["counts"]["generated"] = state["counts"].get("generated", 0) + 1
            mid = mutant_id(fixture, kind, 0)
            mdir = OUT_DIR / "mutants" / mid
            if mdir.exists():
                shutil.rmtree(mdir)
            mdir.mkdir(parents=True)
            build_local = fix_dir / "build.omg"
            if build_local.is_file():
                shutil.copy2(build_local, mdir / "build.omg")
            pkg = fix_dir
            root_tests = REPO / "tests" / "omega"
            while pkg > root_tests and not (pkg / "build.omg").is_file():
                pkg = pkg.parent
            if (pkg / "build.omg").is_file() and pkg != fix_dir:
                shutil.copy2(pkg / "build.omg", mdir / "build.omg")
                for sibling in pkg.glob("*.omg"):
                    if sibling.name != "main.omg":
                        shutil.copy2(sibling, mdir / sibling.name)
            for extra in fix_dir.iterdir():
                if extra.name not in ("main.omg", "build.omg", "expected.txt", "expected_native_exit.txt", "README.md"):
                    if extra.is_file():
                        shutil.copy2(extra, mdir / extra.name)
            (mdir / "main.omg").write_text(mutated)
            ensure_build_omg(mdir, fix_dir, fixture)

            ec, omega_out, t_omega = run_check([str(OMEGA_BIN), "--check", str(mdir / "main.omg")])
            cur = current_verdict(ec, omega_out)

            ec2, route_out, t_route = run_check([str(ROUTE_BIN), str(mdir / "main.omg")])
            route = route_verdict(route_out)
            secs = t_omega + t_route
            state["counts"]["run"] = state["counts"].get("run", 0) + 1

            cur_ok = cur in ("accepted", "rejected", "outside")
            route_ok = route in ("accepted", "rejected", "outside")
            disagree = (
                cur_ok
                and route_ok
                and cur != route
                and cur != "timeout"
                and route != "timeout"
            )
            either_accepts = cur == "accepted" or route == "accepted"
            rec = MutantRecord(fixture, kind, str(mdir.relative_to(REPO)), cur, route, disagree, either_accepts, secs)

            with LOG_TSV.open("a") as f:
                f.write(
                    f"{fixture}\t{kind}\t{rec.mutant_dir}\t{cur}\t{route}\t{disagree}\t{either_accepts}\t{secs:.2f}\n"
                )

            keep = (cur == "accepted" or route == "accepted") or disagree
            if keep:
                candidates.append(rec)
                state["counts"]["kept_candidate"] = state["counts"].get("kept_candidate", 0) + 1
                meta = mdir / "META.json"
                meta.write_text(json.dumps(asdict(rec), indent=2) + "\n")

        done.add(fixture)
        state["done_fixtures"] = sorted(done)
        save_state(state)
        if (fi + 1) % 10 == 0:
            print(f"progress {fi+1}/{len(fixtures)} fixtures", flush=True)

    cand_path = OUT_DIR / "candidates.json"
    cand_path.write_text(json.dumps([asdict(c) for c in candidates], indent=2) + "\n")
    print(f"done fixtures={len(done)} candidates={len(candidates)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
