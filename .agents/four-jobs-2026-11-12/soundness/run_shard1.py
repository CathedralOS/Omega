#!/usr/bin/env python3
"""Job 2 shard 1: generate mutations, run omega --check + route_one_comparison."""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 1
OUT = Path(__file__).resolve().parent
MUT_ROOT = OUT / f"shard-{SHARD}"
TSV = OUT / f"shard-{SHARD}.tsv"
CHECKPOINT = OUT / f"shard-{SHARD}.checkpoint"
STATS = OUT / f"shard-{SHARD}-generation-stats.json"
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
TIMEOUT = 120
ENV = {**os.environ, "OMEGA_LIBRARY_ROOT": str(REPO / "source/library")}
FIXTURE_WORKERS = 2
_IO_LOCK = threading.Lock()

KIND_ORDER = (
    "drop_requires",
    "remove_guard_transition",
    "remove_mut",
    "shared_instead_of_mut",
    "mut_ref_instead_of_shared",
    "widen_requires",
)


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


def requires_is_machine_header(source: str, pos: int) -> bool:
    before = source[:pos]
    starts = [m.start() for m in re.finditer(r"^machine\s+", before, re.MULTILINE)]
    if not starts:
        return False
    return "{" not in source[starts[-1] : pos]


def mutators(source: str) -> list[tuple[str, str, str]]:
    """Return (kind, mutant_source, site_note) for every applicable mutation."""
    out: list[tuple[str, str, str]] = []
    seen_kind: set[str] = set()

    def add(kind: str, text: str, site: str = "") -> None:
        if kind in seen_kind or text == source:
            return
        seen_kind.add(kind)
        out.append((kind, text, site))

    header_req = None
    for m in re.finditer(r"^(\s*)requires\b[^\n]*\n", source, re.MULTILINE):
        if requires_is_machine_header(source, m.start()):
            header_req = m
            break
    if header_req:
        add(
            "drop_requires",
            source[: header_req.start()] + source[header_req.end() :],
            "machine-header",
        )
    else:
        m = re.search(r"^(\s*)requires\b[^\n]*\n", source, re.MULTILINE)
        if m:
            add("drop_requires", source[: m.start()] + source[m.end() :], "body")

    m = re.search(r"^\s*transition\s+[^\n]+\{\s*\n", source, re.MULTILINE)
    if m:
        add("remove_guard_transition", source[: m.start()] + source[m.end() :])

    m = re.search(r"\blet mut (\w+)", source)
    if m:
        name = m.group(1)
        if re.search(
            rf"(?:&mut\s+{re.escape(name)}\b|&write\s+{re.escape(name)}\b|{re.escape(name)}\s*=(?!=)|{re.escape(name)}\[[^\]]+\]\s*=|{re.escape(name)}\.\w+\s*=)",
            source,
        ):
            add("remove_mut", source[: m.start()] + "let " + source[m.end() :])

    m = re.search(r"&mut\b", source)
    if m:
        add("shared_instead_of_mut", source[: m.start()] + "&" + source[m.end() :])

    m = re.search(r"(?<![&])&(?![&])(?=\w)", source)
    if m:
        add("mut_ref_instead_of_shared", source[: m.start()] + "&mut" + source[m.end() :])

    for mo in re.finditer(r"(<=\s*)(\d+)([iuf]?\d*)", source):
        n = int(mo.group(2))
        if n > 0:
            rep = f"{mo.group(1)}{n - 1}{mo.group(3)}"
            add("widen_requires", source[: mo.start()] + rep + source[mo.end() :])
            break

    order = {k: i for i, k in enumerate(KIND_ORDER)}
    out.sort(key=lambda t: order.get(t[0], 99))
    return out


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


def extract_route_reason(raw: str, route: str) -> str:
    parts: list[str] = []
    for line in raw.splitlines():
        if line.startswith("diagnostic\t"):
            parts.append(line.split("\t")[-1].strip()[:240])
        elif line.startswith("fallback\t"):
            parts.append("fallback: " + line.split("\t", 4)[-1].strip()[:200])
        elif line.startswith("current-diagnostic\t"):
            parts.append("current: " + line.split("\t")[-1].strip()[:200])
    for line in raw.splitlines():
        if line.startswith("program\t"):
            tail = "\t".join(line.split("\t")[2:])
            if "Unsupported" in tail or route in ("outside", "no-entry", "unreached"):
                parts.append(tail[:300])
            break
    return " | ".join(parts[:4])[:600]


def run_checks_parallel(main: Path) -> tuple[str, float, str, str, float, str, str]:
    with ThreadPoolExecutor(max_workers=2) as pool:
        f_cur = pool.submit(run_omega, main)
        f_rt = pool.submit(run_route, main)
        cur, cs, c_out = f_cur.result()
        rt, rs, r_out, r_reason = f_rt.result()
    return cur, cs, c_out, rt, rs, r_out, r_reason


def run_route(main: Path) -> tuple[str, float, str, str]:
    t0 = time.monotonic()
    try:
        r = subprocess.run(
            [str(ROUTE), str(main)],
            capture_output=True,
            text=True,
            timeout=TIMEOUT,
        )
    except subprocess.TimeoutExpired as e:
        raw = (e.stdout or "") + (e.stderr or "")
        return "timeout", time.monotonic() - t0, raw[:1200], "timeout"
    route = "error"
    raw = r.stdout or ""
    for line in raw.splitlines():
        if line.startswith("program\t"):
            parts = {kv.split("=", 1)[0]: kv.split("=", 1)[1] for kv in line.split("\t")[2:] if "=" in kv}
            route = parts.get("route", "error")
            break
    reason = extract_route_reason(raw, route)
    return route, time.monotonic() - t0, raw[:1200], reason


def load_checkpoint() -> set[str]:
    if not CHECKPOINT.exists():
        return set()
    done: set[str] = set()
    for ln in CHECKPOINT.read_text().splitlines():
        ln = ln.strip()
        if not ln:
            continue
        done.add(ln)
    return done


def append_checkpoint(key: str) -> None:
    with CHECKPOINT.open("a") as f:
        f.write(key + "\n")


def tsv_header() -> str:
    return (
        "fixture\ttier\tmutation_kind\tmutant_site\tmutant_path\tcurrent_verdict\t"
        "route_verdict\troute_reason\tagree\thand_verdict\tverdict_reason\t"
        "spec_file\tspec_quote\tobligation_line\tcurrent_out\troute_out\t"
        "current_s\troute_s\n"
    )


def process_fixture(
    fixture: str,
    done: set[str],
    tsv,
    stats: dict[str, int | dict[str, int]],
) -> int:
  row_count = 0
  source = (REPO / fixture).read_text(encoding="utf-8")
  muts = mutators(source)
  staging = MUT_ROOT / f"_staging_{slug(fixture)}"
  if muts:
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

  for kind, mutated, site in muts:
    ck = f"{fixture}\t{kind}"
    if ck in done:
      continue
    main_omg.write_text(mutated, encoding="utf-8")
    cur, cs, c_out, rt, rs, r_out, r_reason = run_checks_parallel(main_omg)
    agree = "agree" if cur == rt else "disagree"
    hand = "timeout" if cur == "timeout" or rt == "timeout" else "pending-triage"
    rel = f"{slug(fixture)}__{kind}"
    dest = MUT_ROOT / rel
    if dest.exists():
      shutil.rmtree(dest)
    shutil.copytree(staging, dest)
    dest_rel = str(dest.relative_to(REPO))
    line = (
      f"{fixture}\t{tier_of(fixture)}\t{kind}\t{site}\t{dest_rel}\t{cur}\t{rt}\t"
      f"{r_reason.replace(chr(9),' ').replace(chr(10),' ')}\t{agree}\t{hand}\t\t\t\t\t"
      f"{c_out.replace(chr(9),' ').replace(chr(10),' ')}\t"
      f"{r_out.replace(chr(9),' ').replace(chr(10),' ')}\t{cs:.3f}\t{rs:.3f}\n"
    )
    with _IO_LOCK:
      tsv.write(line)
      tsv.flush()
      append_checkpoint(ck)
      done.add(ck)
      stats["mutants_by_kind"][kind] = stats["mutants_by_kind"].get(kind, 0) + 1
      if kind == "drop_requires" and site == "machine-header":
        stats["drop_requires_machine_header"] += 1
    row_count += 1

  if not muts:
    none_key = f"{fixture}\tnone"
    if none_key not in done:
      line = (
        f"{fixture}\t{tier_of(fixture)}\tnone\t\t\trejected\trejected\t"
        f"no mutation generated\tagree\tpending-triage\t\t\t\t\t\t0\t0\n"
      )
      with _IO_LOCK:
        tsv.write(line)
        tsv.flush()
        append_checkpoint(none_key)
        done.add(none_key)
      row_count += 1
  if muts and staging.exists():
    shutil.rmtree(staging, ignore_errors=True)
  return row_count


def main() -> None:
    fresh = "--fresh" in sys.argv
    if fresh:
        if CHECKPOINT.exists():
            CHECKPOINT.rename(CHECKPOINT.with_suffix(".checkpoint.bak"))
        if TSV.exists():
            TSV.rename(TSV.with_suffix(".tsv.bak"))
    MUT_ROOT.mkdir(parents=True, exist_ok=True)
    fixtures = fixture_list()
    done = load_checkpoint()
    stats = {
        "mutants_by_kind": {},
        "drop_requires_machine_header": 0,
        "fixtures": len(fixtures),
        "mutant_rows_target": 0,
    }
    new_file = not TSV.exists() or TSV.stat().st_size == 0
    with TSV.open("a" if not new_file else "w", encoding="utf-8") as tsv:
        if new_file:
            tsv.write(tsv_header())

        pending_fixtures: list[str] = []
        for fixture in fixtures:
            muts = mutators((REPO / fixture).read_text(encoding="utf-8"))
            if not muts:
                if f"{fixture}\tnone" not in done:
                    pending_fixtures.append(fixture)
                continue
            if any(f"{fixture}\t{kind}" not in done for kind, _, _ in muts):
                pending_fixtures.append(fixture)

        row_count = 0
        stats["mutants_by_kind"] = stats.get("mutants_by_kind") or {}
        if isinstance(stats["drop_requires_machine_header"], int):
            pass
        stats["drop_requires_machine_header"] = stats.get("drop_requires_machine_header", 0)

        completed = 0
        with ThreadPoolExecutor(max_workers=FIXTURE_WORKERS) as pool:
            futures = [
                pool.submit(process_fixture, fixture, done, tsv, stats) for fixture in pending_fixtures
            ]
            for fi, fut in enumerate(futures):
                row_count += fut.result()
                completed += 1
                if completed % 20 == 0 or completed == len(pending_fixtures):
                    print(
                        f"progress fixtures {completed}/{len(pending_fixtures)} rows+={row_count}",
                        flush=True,
                    )

        stats["mutant_rows_written"] = row_count
        STATS.write_text(json.dumps(stats, indent=2) + "\n", encoding="utf-8")
    print(f"finished shard {SHARD} pending_fixtures={len(pending_fixtures)} stats={stats}")


if __name__ == "__main__":
    main()
