#!/usr/bin/env python3
"""Job 2 shard runner: generate mutations, run checkers, append TSV (resumable)."""

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

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = int(os.environ.get("FOUR_JOBS_SHARD", "7"))
SHARD_COUNT = 8
TIMEOUT = int(os.environ.get("MUTATION_CHECK_TIMEOUT", "120"))

OUT_ROOT = REPO / ".agents/four-jobs-2026-11-12/soundness"
MUTANT_ROOT = OUT_ROOT / f"shard-{SHARD}"
TSV_PATH = OUT_ROOT / f"shard-{SHARD}.tsv"
PROGRESS_PATH = OUT_ROOT / f"shard-{SHARD}.progress"

ROUTE_BIN = Path(os.environ.get("ROUTE_ONE", REPO / "target/debug/examples/route_one_comparison"))
OMEGA_BIN = Path(os.environ.get("OMEGA_BIN", REPO / "target/debug/omega"))


def shard_fixtures() -> list[Path]:
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
    return [REPO / p for i, p in enumerate(lines) if i % SHARD_COUNT == SHARD]


def load_done_fixtures() -> set[str]:
    if not PROGRESS_PATH.exists():
        return set()
    return {ln.strip() for ln in PROGRESS_PATH.read_text().splitlines() if ln.strip()}


def append_progress(fixture: str) -> None:
    with PROGRESS_PATH.open("a") as f:
        f.write(fixture + "\n")


def load_tsv_keys() -> set[str]:
    if not TSV_PATH.exists():
        return set()
    keys: set[str] = set()
    for line in TSV_PATH.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) >= 3:
            keys.add(f"{parts[0]}\t{parts[1]}")
    return keys


@dataclass
class Mutation:
    kind: str
    source: str


REQUIRES_RE = re.compile(
    r"^(\s*)(requires\s+)(.+)$", re.MULTILINE
)
ENSURES_RE = re.compile(
    r"^(\s*)(ensures\s+)(.+)$", re.MULTILINE
)
LET_MUT_RE = re.compile(r"\blet\s+mut\s+")
REF_MUT_RE = re.compile(r"&mut\b")
REF_SHARED_RE = re.compile(r"(?<![&])&(?!\s*mut)(?=\s*\w)")


def mutations_for(text: str) -> list[Mutation]:
    out: list[Mutation] = []

    def add(kind: str, new: str) -> None:
        if new != text:
            out.append(Mutation(kind, new))

    # Drop first requires clause entirely (machine-level).
    m = REQUIRES_RE.search(text)
    if m:
        lines = text.splitlines(keepends=True)
        for i, ln in enumerate(lines):
            if REQUIRES_RE.match(ln):
                add("drop_requires", "".join(lines[:i] + lines[i + 1 :]))
                break

    # Widen requires: flip comparison on first requires line.
    if m:
        body = m.group(3)
        widened = body
        for a, b in [(">=", "<"), (">", "<="), ("<=", ">"), ("<", ">="), ("==", "!=")]:
            if a in body:
                widened = body.replace(a, b, 1)
                break
        if widened != body:
            new = text[: m.start(3)] + widened + text[m.end(3) :]
            add("widen_requires", new)

    # Weaken ensures on first machine: replace == with != or drop ensures line.
    em = ENSURES_RE.search(text)
    if em:
        body = em.group(3)
        if "==" in body:
            weakened = body.replace("==", "!=", 1)
            new = text[: em.start(3)] + weakened + text[em.end(3) :]
            add("break_ensures", new)
        lines = text.splitlines(keepends=True)
        for i, ln in enumerate(lines):
            if ENSURES_RE.match(ln):
                add("drop_ensures", "".join(lines[:i] + lines[i + 1 :]))
                break

    if LET_MUT_RE.search(text):
        add("drop_mut", LET_MUT_RE.sub("let ", text, count=1))

    if REF_MUT_RE.search(text):
        add("shared_instead_of_mut", REF_MUT_RE.sub("&", text, count=1))

    if REF_SHARED_RE.search(text) and ("=" in text or ".value" in text):
        add("mut_ref_instead_of_shared", REF_SHARED_RE.sub("&mut ", text, count=1))

    # Delete a simple guard line: `if ... {` before obvious dangerous ops.
    guard = re.compile(r"^\s*if\s+.+\{\s*$", re.MULTILINE)
    gm = guard.search(text)
    if gm and re.search(r"/\s|/\w|\[\s*\w", text):
        lines = text.splitlines(keepends=True)
        for i, ln in enumerate(lines):
            if guard.match(ln):
                add("drop_guard", "".join(lines[:i] + lines[i + 1 :]))
                break

    # Deduplicate by source hash
    seen: set[str] = set()
    unique: list[Mutation] = []
    for mut in out:
        h = hashlib.sha256(mut.source.encode()).hexdigest()[:16]
        if h in seen:
            continue
        seen.add(h)
        unique.append(mut)
    return unique[:12]


def mutant_dir(fixture_rel: str, kind: str, digest: str) -> Path:
    safe = fixture_rel.replace("/", "__")
    return MUTANT_ROOT / safe / f"{kind}_{digest}"


def materialize_mutant(fixture_main: Path, mut: Mutation) -> Path:
    fixture_dir = fixture_main.parent
    rel = fixture_main.relative_to(REPO).as_posix()
    digest = hashlib.sha256(mut.source.encode()).hexdigest()[:10]
    dest = mutant_dir(rel, mut.kind, digest)
    if dest.exists():
        return dest / "main.omg"
    dest.mkdir(parents=True, exist_ok=True)
    for item in fixture_dir.iterdir():
        if item.name == "main.omg":
            continue
        if item.is_file():
            shutil.copy2(item, dest / item.name)
    (dest / "main.omg").write_text(mut.source)
    return dest / "main.omg"


def run_omega_check(main: Path) -> tuple[str, float]:
    start = time.monotonic()
    try:
        proc = subprocess.run(
            [str(OMEGA_BIN), "--check", str(main)],
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
            stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        return "timeout", time.monotonic() - start
    elapsed = time.monotonic() - start
    if proc.returncode == 0:
        return "accepted", elapsed
    return "rejected", elapsed


def parse_route(stdout: str) -> tuple[str, str]:
    current = route = "missing"
    for line in stdout.splitlines():
        if not line.startswith("program\t"):
            continue
        fields = line.split("\t")
        for f in fields:
            if f.startswith("current="):
                current = f.split("=", 1)[1]
            if f.startswith("route="):
                route = f.split("=", 1)[1]
    return current, route


def run_route(main: Path) -> tuple[str, str, float]:
    start = time.monotonic()
    try:
        proc = subprocess.run(
            [str(ROUTE_BIN), str(main)],
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
            stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        return "timeout", "timeout", time.monotonic() - start
    elapsed = time.monotonic() - start
    if proc.returncode != 0 and not proc.stdout:
        return "error", "error", elapsed
    cur, route = parse_route(proc.stdout)
    return cur, route, elapsed


def append_tsv(row: list[str]) -> None:
    header = (
        "original_fixture\tmutation_kind\tmutant_path\tomega_verdict\t"
        "route_current_verdict\troute_verdict\tseconds\tcandidate\n"
    )
    if not TSV_PATH.exists():
        TSV_PATH.write_text(
            "# pin=" + PIN + f" shard={SHARD}\n" + header
        )
    with TSV_PATH.open("a") as f:
        f.write("\t".join(row) + "\n")


def main() -> int:
    if not ROUTE_BIN.is_file() or not OMEGA_BIN.is_file():
        print("missing binaries", ROUTE_BIN, OMEGA_BIN, file=sys.stderr)
        return 2

    fixtures = shard_fixtures()
    done = load_done_fixtures()
    print(f"shard {SHARD}: {len(fixtures)} fixtures, {len(done)} done", flush=True)

    counts = {"generated": 0, "run": 0, "candidate": 0}

    for main in fixtures:
        rel = main.relative_to(REPO).as_posix()
        if rel in done:
            continue
        text = main.read_text()
        muts = mutations_for(text)
        counts["generated"] += len(muts)
        for mut in muts:
            mpath = materialize_mutant(main, mut)
            cur_v, route_v, route_s = run_route(mpath)
            omg_v = "skipped"
            omg_s = 0.0
            needs_omega = (
                cur_v == "accepted"
                or route_v == "accepted"
                or (cur_v in ("accepted", "rejected") and route_v in ("accepted", "rejected", "incomplete", "fallback") and cur_v != route_v)
            )
            if needs_omega:
                omg_v, omg_s = run_omega_check(mpath)
            counts["run"] += 1
            accept = omg_v == "accepted" or cur_v == "accepted" or route_v == "accepted"
            disagree = (
                omg_v in ("accepted", "rejected")
                and cur_v in ("accepted", "rejected")
                and omg_v != cur_v
            ) or (
                cur_v in ("accepted", "rejected")
                and route_v in ("accepted", "rejected", "incomplete", "fallback")
                and cur_v != route_v
            )
            candidate = accept or disagree
            if candidate:
                counts["candidate"] += 1
            append_tsv(
                [
                    rel,
                    mut.kind,
                    mpath.relative_to(REPO).as_posix(),
                    omg_v,
                    cur_v,
                    route_v,
                    f"{max(omg_s, route_s):.2f}",
                    "yes" if candidate else "no",
                ]
            )
        append_progress(rel)

    stats_path = OUT_ROOT / f"shard-{SHARD}.counts"
    stats_path.write_text(
        f"generated={counts['generated']}\nrun={counts['run']}\ncandidate={counts['candidate']}\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
