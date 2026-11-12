#!/usr/bin/env python3
"""Job 1 shard runner: route coverage measurement (shard 3 of 8)."""
from __future__ import annotations

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

FALLBACK_UNDECIDED = ("fallback", "refused-module", "defect", "panicked")
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
    if raw.startswith("reason="):
        return raw.removeprefix("reason=").strip()
    return raw


def outside_payload(fields: list[str]) -> str:
    try:
        idx = fields.index("route=outside")
    except ValueError:
        return ""
    if idx + 1 < len(fields):
        return fields[idx + 1]
    return ""


def parse_program_line(line: str) -> dict[str, str]:
    fields = line.split("\t")
    if len(fields) < 2:
        return {}
    fm = field_map(fields[2:])
    return {
        "fixture": fields[1],
        "current": fm.get("current", ""),
        "route": fm.get("route", ""),
        "root": fm.get("root", ""),
        "outside": outside_payload(fields),
        "refused_reason": fm.get("reason", ""),
        "refused_detail": fields[-1] if fm.get("route") == "refused-module" else "",
    }


JUDGED_ROUTES = frozenset({"accepted", "rejected"})


def route_tsv_verdict(routes: list[str]) -> str:
    """All roots must be judged; partial when any root accepts alongside unjudged/outside."""
    if not routes:
        return "error"
    any_accepted = any(r == "accepted" for r in routes)
    any_rejected = any(r == "rejected" for r in routes)
    any_non_judged = any(r not in JUDGED_ROUTES for r in routes)

    if any_accepted and any_non_judged:
        return "partial"
    if any_rejected:
        return "rejected"
    if any_accepted:
        return "accepted"
    return "outside"


def pick_first_refusal(
    route_verdict: str,
    programs: list[dict[str, str]],
    fallbacks: list[str],
) -> str:
    """Construct or bucket name for ranking (not generic Unsupported)."""
    if route_verdict == "rejected":
        return "-"
    if route_verdict == "partial":
        for prog in programs:
            route = prog.get("route", "")
            if route == "accepted":
                continue
            if route == "outside" and prog.get("outside"):
                return f"partial:{normalize_construct_refusal(prog['outside'])}"
            if route not in JUDGED_ROUTES:
                return f"partial:{normalize_construct_refusal(route)}"
        return "partial:unclassified"

    for prog in programs:
        route = prog.get("route", "")
        if route == "outside" and prog.get("outside"):
            return normalize_construct_refusal(prog["outside"])
        if route == "refused-module" and prog.get("refused_reason"):
            detail = prog.get("refused_detail", "")
            base = normalize_construct_refusal(prog["refused_reason"])
            if detail and detail != base:
                return f"{base} ({detail[:120]})"
            return base
        if route == "no-entry":
            return "bucket:no-program-entry"
        if route == "unreached":
            return "bucket:current-checker-stopped"
        if route == "incomplete":
            return "bucket:prover-incomplete"

    if fallbacks:
        return normalize_construct_refusal(fallbacks[0])

    if route_verdict in ("outside", "accepted"):
        return "-"
    return "-"


def agreement(current: str, route_verdict: str) -> str:
    if route_verdict not in ("accepted", "rejected"):
        return "n-a"
    if current not in ("accepted", "rejected"):
        return "n-a"
    cur_ok = current == "accepted"
    route_ok = route_verdict == "accepted"
    return "agree" if cur_ok == route_ok else "disagree"


def run_one(fixture: str) -> tuple[str, str, str, str, float]:
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
        return "error", "", "n-a", str(exc)[:200], sec

    sec = time.monotonic() - t0
    if proc.returncode == 124:
        return "timeout", "", "n-a", "", sec

    out = proc.stdout or ""
    err = proc.stderr or ""
    if proc.returncode != 0 and not out.strip():
        detail = (err or out).strip().split("\n")[0][:200]
        return "error", "", "n-a", detail, sec

    programs: list[dict[str, str]] = []
    fallbacks: list[str] = []
    current = ""

    for line in out.splitlines():
        if line.startswith("program\t"):
            parsed = parse_program_line(line)
            if parsed.get("fixture") in (rel, fixture) or rel.endswith(
                parsed.get("fixture", "")
            ):
                programs.append(parsed)
                if parsed.get("current"):
                    current = parsed["current"]
        elif line.startswith("fallback\t") and rel in line:
            parts = line.split("\t")
            if len(parts) >= 4:
                fallbacks.append(parts[3].strip())

    if not programs:
        if "panicked" in out:
            return "error", "panicked", "n-a", "panicked", sec
        return "error", current or "", "n-a", "no-output", sec

    routes = [p["route"] for p in programs if p.get("route")]
    route_verdict = route_tsv_verdict(routes)
    first_refusal = pick_first_refusal(route_verdict, programs, fallbacks)
    agr = agreement(current or programs[0].get("current", ""), route_verdict)
    return (
        route_verdict,
        current or programs[0].get("current", ""),
        agr,
        first_refusal,
        sec,
    )


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
