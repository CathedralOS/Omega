#!/usr/bin/env python3
"""Job 2 shard 1: generate mutations, run omega --check + route_one_comparison."""
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
SHARD = 1
OUT = Path(__file__).resolve().parent
MUT_ROOT = OUT / f"shard-{SHARD}"
TSV = OUT / f"shard-{SHARD}.tsv"
CHECKPOINT = OUT / f"shard-{SHARD}.checkpoint"
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
TIMEOUT = 120
ENV = {**os.environ, "OMEGA_LIBRARY_ROOT": str(REPO / "source/library")}


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
    h = hashlib.sha256(s.encode()).hexdigest()[:12]
    leaf = s.rsplit("/", 2)[-2] if s.count("/") >= 2 else Path(s).stem
    safe = re.sub(r"[^a-zA-Z0-9_]+", "_", leaf)[:48]
    return f"{safe}_{h}"


def copy_fixture(fixture_rel: str, dest: Path) -> None:
    src_dir = REPO / Path(fixture_rel).parent
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(src_dir, dest)


def mutators(source: str) -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []

    def add(kind: str, text: str) -> None:
        if text != source:
            out.append((kind, text))

    m = re.search(r"^(\s*)requires\b[^\n]*\n", source, re.MULTILINE)
    if m:
        add("drop_requires", source[: m.start()] + source[m.end() :])

    def widen(mo: re.Match[str]) -> None:
        n = int(mo.group(2))
        if n > 0:
            rep = f"{mo.group(1)}{n - 1}{mo.group(3)}"
            add("widen_requires", source[: mo.start()] + rep + source[mo.end() :])

    for mo in re.finditer(r"(<=\s*)(\d+)([iuf]?\d*)", source):
        widen(mo)
        break

    m = re.search(r"^\s*transition\s+[^\n]+\{\s*\n", source, re.MULTILINE)
    if m:
        add("remove_guard_transition", source[: m.start()] + source[m.end() :])

    m = re.search(r"(?<![&])\&(?![&])(?=\w)", source)
    if m:
        add("amp_to_ampmut", source[: m.start()] + "&mut" + source[m.end() :])

    m = re.search(r"\blet mut (\w+)", source)
    if m:
        name = m.group(1)
        if re.search(
            rf"(?:&mut\s+{re.escape(name)}\b|&write\s+{re.escape(name)}\b|{re.escape(name)}\s*=(?!=)|{re.escape(name)}\[[^\]]+\]\s*=|{re.escape(name)}\.\w+\s*=)",
            source,
        ):
            add("remove_mut", source[: m.start()] + "let " + source[m.end() :])

    priority = [
        "drop_requires",
        "remove_guard_transition",
        "remove_mut",
        "amp_to_ampmut",
        "widen_requires",
    ]
    by_kind = {k: t for k, t in out}
    return [(kind, by_kind[kind]) for kind in priority if kind in by_kind][:1]


def run_omega(main: Path) -> tuple[str, float, str]:
    t0 = time.monotonic()
    try:
        r = subprocess.run(
            [str(OMEGA), "--check", str(main)],
            env=ENV,
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
        )
    except subprocess.TimeoutExpired as e:
        out = (e.stdout or "") + (e.stderr or "")
        return "timeout", time.monotonic() - t0, out[:800]
    out = (r.stdout or "") + (r.stderr or "")
    verdict = "accepted" if r.returncode == 0 else "rejected"
    return verdict, time.monotonic() - t0, out[:800]


def run_route(main: Path) -> tuple[str, float, str]:
    t0 = time.monotonic()
    try:
        r = subprocess.run(
            [str(ROUTE), str(main)],
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
        )
    except subprocess.TimeoutExpired as e:
        return "timeout", time.monotonic() - t0, (e.stdout or "")[:800]
    route = "error"
    raw = r.stdout or ""
    for line in raw.splitlines():
        if line.startswith("program\t"):
            parts = {kv.split("=", 1)[0]: kv.split("=", 1)[1] for kv in line.split("\t")[2:] if "=" in kv}
            route = parts.get("route", "error")
            break
    return route, time.monotonic() - t0, raw[:800]


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
    done = load_checkpoint()
    write_header = not TSV.exists() or TSV.stat().st_size == 0
    mode = "a" if TSV.exists() and not write_header else "w"

    with TSV.open(mode, encoding="utf-8") as tsv:
        if write_header:
            tsv.write(
                "fixture\ttier\tmutation_kind\tmutant_path\tcurrent_verdict\t"
                "route_verdict\tagree\thand_verdict\tspec_file\tspec_quote\t"
                "obligation_line\tcurrent_out\troute_out\tcurrent_s\troute_s\n"
            )

        for fi, fixture in enumerate(fixtures):
            if fixture in done:
                continue
            source = (REPO / fixture).read_text(encoding="utf-8")
            muts = mutators(source)
            staging = MUT_ROOT / f"_staging_{slug(fixture)}"
            copy_fixture(fixture, staging)
            main_omg = staging / "main.omg"
            if not (staging / "build.omg").is_file() and re.search(r"(?m)^machine main\(", source):
                build = f"""machine build(builder: &mut Build) {{
    Build::application(builder, "{Path(fixture).parent.name}");
    Build::bind(builder, macos_arm64::ProgramEntry, main);
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
    Build::bind(builder, linux_arm64::ProgramEntry, main);
    Build::bind(builder, windows_x86_64::ProgramEntry, main);
}}
"""
                (staging / "build.omg").write_text(build)

            for kind, mutated in muts:
                main_omg.write_text(mutated, encoding="utf-8")
                cur, cs, c_out = run_omega(main_omg)
                rt, rs, r_out = run_route(main_omg)
                agree = "agree" if cur == rt else "disagree"
                if cur == "timeout" or rt == "timeout":
                    hand = "timeout"
                else:
                    hand = "pending-triage"
                dest_rel = ""
                rel = f"{slug(fixture)}__{kind}"
                dest = MUT_ROOT / rel
                if dest.exists():
                    shutil.rmtree(dest)
                shutil.copytree(staging, dest)
                dest_rel = str(dest.relative_to(REPO))
                tsv.write(
                    f"{fixture}\t{tier_of(fixture)}\t{kind}\t{dest_rel}\t{cur}\t{rt}\t"
                    f"{agree}\t{hand}\t\t\t\t{c_out.replace(chr(9),' ').replace(chr(10),' ')}\t"
                    f"{r_out.replace(chr(9),' ').replace(chr(10),' ')}\t{cs:.3f}\t{rs:.3f}\n"
                )
                tsv.flush()
            if not muts:
                tsv.write(
                    f"{fixture}\t{tier_of(fixture)}\tnone\t\trejected\trejected\tagree\t"
                    f"discarded-no-mutation\t\t\t\t\t\t0\t0\n"
                )
                tsv.flush()
            shutil.rmtree(staging, ignore_errors=True)
            append_checkpoint(fixture)
            if (fi + 1) % 25 == 0:
                print(f"progress {fi+1}/{len(fixtures)}", flush=True)
                subprocess.run(["git", "-C", str(REPO), "add", str(TSV), str(CHECKPOINT)], check=False)
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
                    ["git", "-C", str(REPO), "push", "origin", f"work/soundness-mutations-shard-{SHARD}"],
                    check=False,
                )

    print(f"finished shard {SHARD} fixtures={len(fixtures)}")


if __name__ == "__main__":
    main()
