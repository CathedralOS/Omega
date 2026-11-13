#!/usr/bin/env python3
"""Job 1 shard runner: route coverage measurement (shard 3 of 8)."""
from __future__ import annotations

import hashlib
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
BIN = REPO / "target/debug/examples/route_one_comparison"
OUT_DIR = Path(__file__).resolve().parent
TSV = OUT_DIR / "shard-3.tsv"
RAW_DIR = Path("/tmp/four-jobs-route-coverage-shard-3")
SHARD = 3
SHARDS = 8
TIMEOUT_S = 300
HEADER = (
    "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\t"
    "roots\tvacuous_accept_roots\tseconds\n"
)

REASON_PREFIXES = (
    "Unsupported: ",
    "Incomplete: ",
    "IllFormed: ",
    "OutOfScope: ",
)


def tier(fixture: str) -> str:
    for t in ("pass", "run", "fail"):
        if f"/{t}/" in fixture:
            return t
    return "unknown"


def fixture_universe() -> list[str]:
    out = subprocess.check_output(
        [
            "git",
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
            "tests/omega/fail/**/main.omg",
        ],
        cwd=REPO,
        text=True,
    )
    lines = sorted(out.splitlines(), key=lambda s: s)
    return [line for i, line in enumerate(lines) if i % SHARDS == SHARD]


def load_done() -> set[str]:
    if not TSV.exists():
        return set()
    done: set[str] = set()
    for line in TSV.read_text().splitlines()[1:]:
        if line.strip():
            done.add(line.split("\t", 1)[0])
    return done


def field_map(fields: list[str]) -> dict[str, str]:
    out: dict[str, str] = {}
    for f in fields:
        if f.startswith("root="):
            out["root"] = f.removeprefix("root=")
        elif "=" in f:
            k, v = f.split("=", 1)
            out[k] = v
    return out


def normalize_construct_refusal(raw: str) -> str:
    raw = raw.strip()
    if not raw:
        return ""
    if raw == "no-entry":
        return "bucket:no-program-entry"
    if raw == "unreached":
        return "bucket:current-checker-stopped"
    for prefix in REASON_PREFIXES:
        if raw.startswith(prefix):
            return raw.removeprefix(prefix).strip()
    return raw


def outside_payload(fields: list[str]) -> str:
    try:
        idx = fields.index("route=outside")
    except ValueError:
        return ""
    if idx + 1 < len(fields):
        return fields[idx + 1]
    return ""


def root_verdict_token(route: str) -> str:
    """Per-root verdict tokens for merge (fixed vocabulary)."""
    if route in ("accepted", "rejected", "no-entry", "outside", "error", "timeout"):
        return route
    if route == "unreached":
        return "no-entry"
    if route in (
        "incomplete",
        "fallback",
        "refused-module",
        "defect",
        "panicked",
    ):
        return "outside"
    return "outside"


def root_column_label(root: str) -> str:
    if root == "compilation":
        return "compilation"
    if root.startswith("machine:"):
        return root
    return root


def parse_program_line(line: str) -> dict[str, str | int]:
    fields = line.split("\t")
    if len(fields) < 2:
        return {}
    fm = field_map(fields[2:])
    try:
        obligations = int(fm.get("obligations", "0") or 0)
    except ValueError:
        obligations = 0
    try:
        judged = int(fm.get("judged", "0") or 0)
    except ValueError:
        judged = 0
    return {
        "fixture": fields[1],
        "current": fm.get("current", ""),
        "route": fm.get("route", ""),
        "root": fm.get("root", ""),
        "outside": outside_payload(fields),
        "obligations": obligations,
        "judged": judged,
    }


def format_roots(programs: list[dict[str, str | int]]) -> tuple[str, str, list[str]]:
    parts: list[str] = []
    vacuous: list[str] = []
    per_root_verdicts: list[str] = []
    for prog in programs:
        root = str(prog.get("root", ""))
        route = str(prog.get("route", ""))
        token = root_verdict_token(route)
        label = root_column_label(root)
        parts.append(f"{label}:{token}")
        per_root_verdicts.append(token)
        if token == "accepted":
            ob = int(prog.get("obligations", 0))
            jd = int(prog.get("judged", 0))
            if ob == 0 and jd == 0:
                vacuous.append(label)
    roots_str = ";".join(parts)
    vacuous_str = ";".join(vacuous)
    return roots_str, vacuous_str, per_root_verdicts


def program_verdict(per_root: list[str]) -> str:
    if not per_root:
        return "error"
    if any(v == "rejected" for v in per_root):
        return "rejected"
    if all(v == "accepted" for v in per_root):
        return "accepted"
    if any(v == "accepted" for v in per_root):
        return "partial"
    return "outside"


def pick_first_refusal(
    route_verdict: str,
    programs: list[dict[str, str | int]],
) -> str:
    judged = frozenset({"accepted", "rejected"})

    if route_verdict == "rejected":
        return "-"
    if route_verdict == "partial":
        for prog in programs:
            route = str(prog.get("route", ""))
            if route == "accepted":
                continue
            if route == "outside" and prog.get("outside"):
                return f"partial:{normalize_construct_refusal(str(prog['outside']))}"
            tok = root_verdict_token(route)
            if tok == "no-entry":
                return "bucket:no-program-entry"
            if route == "unreached":
                return "bucket:current-checker-stopped"
            if tok == "outside":
                return f"partial:{normalize_construct_refusal(str(prog.get('outside') or route))}"
        return "partial:unclassified"

    for prog in programs:
        route = str(prog.get("route", ""))
        if route == "outside" and prog.get("outside"):
            return normalize_construct_refusal(str(prog["outside"]))
        if route == "no-entry":
            return "bucket:no-program-entry"
        if route == "unreached":
            return "bucket:current-checker-stopped"
        if route == "incomplete":
            return "bucket:prover-incomplete"

    if route_verdict in ("outside", "accepted"):
        return "-"
    return "-"


def agreement(current: str, route_verdict: str) -> str:
    if route_verdict not in ("accepted", "rejected"):
        return "n-a"
    if current not in ("accepted", "rejected"):
        return "n-a"
    return "agree" if (current == "accepted") == (route_verdict == "accepted") else "disagree"


def raw_path(fixture: str) -> Path:
    RAW_DIR.mkdir(parents=True, exist_ok=True)
    h = hashlib.sha256(fixture.encode()).hexdigest()[:16]
    safe = fixture.replace("/", "__")
    return RAW_DIR / f"{h}_{safe}.txt"


def run_one(
    fixture: str,
) -> tuple[str, str, str, str, str, str, float]:
    rel = fixture
    args = [str(BIN)]
    if tier(fixture) == "fail":
        args.append("--every-machine")
    args.append(rel)
    cmd = ["timeout", str(TIMEOUT_S)] + args
    t0 = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            cwd=REPO,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        )
    except Exception as exc:  # noqa: BLE001
        sec = time.monotonic() - t0
        return "error", "", "n-a", str(exc)[:200], "", "", sec

    sec = time.monotonic() - t0
    out = proc.stdout or ""
    err = proc.stderr or ""
    raw_path(fixture).write_text(out + err, encoding="utf-8")

    if proc.returncode == 124:
        return "timeout", "", "n-a", "", "", "", sec

    if proc.returncode != 0 and not out.strip():
        detail = (err or out).strip().split("\n")[0][:200]
        return "error", "", "n-a", detail, "", "", sec

    programs: list[dict[str, str | int]] = []
    current = ""

    for line in out.splitlines():
        if line.startswith("program\t"):
            parsed = parse_program_line(line)
            if not parsed:
                continue
            fix = str(parsed.get("fixture", ""))
            if fix == rel or rel.endswith(fix):
                programs.append(parsed)
                if parsed.get("current"):
                    current = str(parsed["current"])

    if not programs:
        if "panicked" in out:
            return "error", "panicked", "n-a", "panicked", "", "", sec
        return "error", current or "", "n-a", "no-output", "", "", sec

    roots_str, vacuous_str, per_root = format_roots(programs)
    route_verdict = program_verdict(per_root)
    first_refusal = pick_first_refusal(route_verdict, programs)
    agr = agreement(current or str(programs[0].get("current", "")), route_verdict)
    return (
        route_verdict,
        current or str(programs[0].get("current", "")),
        agr,
        first_refusal,
        roots_str,
        vacuous_str,
        sec,
    )


def append_row(
    fixture: str,
    route_tsv: str,
    current: str,
    agr: str,
    refusal: str,
    roots: str,
    vacuous: str,
    sec: float,
) -> None:
    row = (
        f"{fixture}\t{tier(fixture)}\t{route_tsv}\t{current}\t{agr}\t"
        f"{refusal.replace(chr(9), ' ')}"
        f"\t{roots}\t{vacuous}\t{sec:.3f}\n"
    )
    with TSV.open("a", encoding="utf-8") as f:
        f.write(row)


def main() -> int:
    if not BIN.is_file():
        print(f"missing binary {BIN}", file=sys.stderr)
        return 1
    fixtures = fixture_universe()
    done = load_done()
    if not TSV.exists():
        TSV.write_text(HEADER, encoding="utf-8")
    remaining = [f for f in fixtures if f not in done]
    print(f"shard {SHARD}/{SHARDS}: {len(fixtures)} total, {len(remaining)} remaining")
    for i, fixture in enumerate(remaining, 1):
        row = run_one(fixture)
        append_row(fixture, *row)
        if i % 25 == 0 or i == len(remaining):
            print(f"[{i}/{len(remaining)}] last={fixture} route={row[0]} {row[-1]:.1f}s")
            sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
