#!/usr/bin/env python3
"""Job 1 shard runner: route coverage measurement (shard 3 of 8)."""
from __future__ import annotations

import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
BIN = REPO / "target/debug/examples/route_one_comparison"
OUT_DIR = Path(__file__).resolve().parent
TSV = OUT_DIR / "shard-3.tsv"
SHARD = 3
SHARDS = 8
TIMEOUT_S = 300
HEADER = (
    "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n"
)

UNDECIDED = ("fallback", "refused-module", "defect", "panicked", "no-entry", "unreached")


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
        if "=" in f and not f.startswith("root="):
            k, v = f.split("=", 1)
            out[k] = v
        elif f.startswith("root="):
            out["root"] = f.removeprefix("root=")
    return out


def parse_program_line(line: str) -> tuple[str, str, str, str | None]:
    """Return (fixture, current, route_kind, refusal_hint)."""
    fields = line.split("\t")
    if len(fields) < 2:
        return "", "", "", None
    fixture = fields[1]
    fm = field_map(fields[2:])
    current = fm.get("current", "")
    route = fm.get("route", "")
    refusal: str | None = None
    if route == "outside":
        # Reason is the first bare field after route=outside tab fields.
        try:
            idx = fields.index("route=outside")
            if idx + 1 < len(fields):
                refusal = fields[idx + 1]
        except ValueError:
            pass
    elif route in UNDECIDED and route in ("no-entry", "unreached"):
        refusal = route
    return fixture, current, route, refusal


def aggregate_routes(
    programs: list[tuple[str, str, str, str | None]],
    fallbacks: list[str],
    diagnostics_incomplete: bool,
) -> tuple[str, str]:
    routes = [r for _, _, r, _ in programs if r]
    refusal = ""

    def has(prefix: str) -> bool:
        return any(r == prefix or r.startswith(prefix) for r in routes)

    def first(prefix: str) -> str | None:
        for r in routes:
            if r == prefix or r.startswith(prefix):
                return r
        return None

    for decided in ("rejected", "incomplete"):
        if has(decided):
            if decided == "rejected":
                tsv = "rejected"
            else:
                tsv = "outside"
                if not refusal:
                    refusal = "incomplete"
            break
    else:
        for und in UNDECIDED:
            if first(und):
                tsv = "outside"
                if not refusal:
                    refusal = und
                break
        else:
            if has("accepted"):
                tsv = "accepted"
            elif has("outside"):
                tsv = "outside"
                for _, _, _, hint in programs:
                    if hint:
                        refusal = hint
                        break
            else:
                tsv = "outside"
                if routes:
                    refusal = routes[0]
                else:
                    refusal = "no-program-line"

    if fallbacks and not refusal:
        refusal = fallbacks[0]
    elif diagnostics_incomplete and tsv == "outside" and not refusal:
        refusal = "incomplete"

    for _, _, _, hint in programs:
        if hint and tsv == "outside" and not refusal:
            refusal = hint
            break

    if not refusal and tsv == "outside":
        refusal = "-"

    return tsv, refusal


def agreement(current: str, route_tsv: str) -> str:
    if route_tsv in ("timeout", "error", "outside"):
        return "n-a"
    if current in ("stopped", "") or route_tsv in ("",):
        return "n-a"
    if current == "rejected" and route_tsv == "rejected":
        return "agree"
    if current == "accepted" and route_tsv == "accepted":
        return "agree"
    if current in ("accepted", "rejected") and route_tsv in ("accepted", "rejected"):
        return "disagree"
    return "n-a"


def run_one(fixture: str) -> tuple[str, str, str, str, str, float]:
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
        return "error", "", "n-a", str(exc), sec

    sec = time.monotonic() - t0
    if proc.returncode == 124:
        return "timeout", "", "n-a", "", sec

    out = proc.stdout or ""
    err = proc.stderr or ""
    if "panicked" in out.splitlines()[0:3] if out else []:
        pass

    if proc.returncode != 0 and not out.strip():
        detail = (err or out).strip().split("\n")[0][:200]
        return "error", "", "n-a", detail, sec

    programs: list[tuple[str, str, str, str | None]] = []
    fallbacks: list[str] = []
    current = ""
    diagnostic_incomplete = False

    for line in out.splitlines():
        if line.startswith("program\t"):
            fix, cur, route, hint = parse_program_line(line)
            if fix == rel or fix.endswith(rel) or rel.endswith(fix):
                programs.append((fix, cur, route, hint))
                if cur:
                    current = cur
        elif line.startswith("fallback\t") and rel in line:
            parts = line.split("\t")
            if len(parts) >= 4:
                # fallback constructs are Debug-formatted in field 4.
                construct = parts[3].strip()
                fallbacks.append(construct)
        elif line.startswith("diagnostic\t") and rel in line:
            if "\tincomplete\t" in line:
                diagnostic_incomplete = True

    if not programs:
        if "panicked" in out:
            return "error", "panicked", "n-a", "panicked", sec
        return "error", current or "", "n-a", "no-output", sec

    route_tsv, first_refusal = aggregate_routes(
        programs, fallbacks, diagnostic_incomplete
    )
    agr = agreement(current or programs[0][1], route_tsv)
    return route_tsv, current or programs[0][1], agr, first_refusal, sec


def append_row(
    fixture: str,
    route_tsv: str,
    current: str,
    agr: str,
    refusal: str,
    sec: float,
) -> None:
    row = (
        f"{fixture}\t{tier(fixture)}\t{route_tsv}\t{current}\t{agr}\t"
        f"{refusal.replace(chr(9), ' ')}\t{sec:.3f}\n"
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
        route_tsv, current, agr, refusal, sec = run_one(fixture)
        append_row(fixture, route_tsv, current, agr, refusal, sec)
        if i % 25 == 0 or i == len(remaining):
            print(f"[{i}/{len(remaining)}] last={fixture} route={route_tsv} {sec:.1f}s")
            sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
