#!/usr/bin/env python3
"""Run route_one_comparison on one shard; append TSV rows (resumable)."""

from __future__ import annotations

import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HARNESS = REPO / "target/debug/examples/route_one_comparison"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 5
SHARD_MOD = 8
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
TIMEOUT_SEC = 300

JUDGED_ROUTE = frozenset({"accepted", "rejected"})
OUTSIDE_ROUTE = frozenset(
    {
        "outside",
        "no-entry",
        "unreached",
        "incomplete",
        "fallback",
        "refused-module",
        "defect",
        "panicked",
    }
)


def tier(fixture: str) -> str:
    if fixture.startswith("tests/omega/pass/"):
        return "pass"
    if fixture.startswith("tests/omega/run/"):
        return "run"
    if fixture.startswith("tests/omega/fail/"):
        return "fail"
    return "unknown"


def list_shard_fixtures() -> list[str]:
    proc = subprocess.run(
        "git ls-files 'tests/omega/pass/**/main.omg' "
        "'tests/omega/run/**/main.omg' 'tests/omega/fail/**/main.omg' | LC_ALL=C sort",
        shell=True,
        capture_output=True,
        text=True,
        check=True,
        cwd=REPO,
    )
    lines = sorted(proc.stdout.splitlines())
    return [line for i, line in enumerate(lines) if i % SHARD_MOD == SHARD]


def field_map(line: str, *, body_only: bool = False) -> dict[str, str]:
    parts = line.split("\t")
    out: dict[str, str] = {}
    i = 0 if body_only else 2
    while i < len(parts):
        part = parts[i]
        if "=" in part and not part.startswith("route="):
            key, _, val = part.partition("=")
            out[key] = val
            i += 1
        elif part.startswith("route="):
            route_val = part[len("route=") :]
            if route_val in JUDGED_ROUTE:
                out["route"] = route_val
                i += 1
            elif route_val == "outside":
                out["route"] = "outside"
                if i + 1 < len(parts) and "=" not in parts[i + 1]:
                    out["outside_detail"] = parts[i + 1]
                    i += 2
                else:
                    i += 1
            elif route_val in OUTSIDE_ROUTE:
                out["route"] = route_val
                i += 1
            else:
                out["route"] = "outside"
                if i + 1 < len(parts):
                    out["outside_detail"] = parts[i + 1]
                i += 2
        else:
            i += 1
    if "root" not in out and len(parts) > 2:
        for p in parts[2:]:
            if p.startswith("root="):
                out["root"] = p.split("=", 1)[1]
    return out


def refusal_construct_from_detail(detail: str) -> str:
    """Map harness refusal text to the construct/family string structure_elaboration names."""
    detail = detail.strip()
    if not detail:
        return ""
    if ":" in detail:
        _reason, payload = detail.split(":", 1)
        return payload.strip()
    return detail


def fallback_construct(line: str) -> str:
    parts = line.split("\t")
    if len(parts) < 5:
        return ""
    concerns = parts[4].strip()
    # Rust Debug of `Construct` — keep the enum label for tallying.
    if "(" in concerns:
        return concerns.split("(", 1)[0].strip()
    return concerns


def first_refusal_from_parsed(parsed: list[dict[str, str]], fallbacks: list[str]) -> str:
    for p in parsed:
        route = p.get("route", "")
        if route == "outside" and p.get("outside_detail"):
            construct = refusal_construct_from_detail(p["outside_detail"])
            if construct:
                return construct
        if route == "refused-module":
            reason = p.get("reason", "")
            if reason:
                return reason
    for line in fallbacks:
        construct = fallback_construct(line)
        if construct:
            return construct
    for p in parsed:
        route = p.get("route", "")
        if route == "no-entry":
            return "no_program_entry"
        if route == "unreached":
            return "current_stopped_before_typed_trees"
    return ""


def aggregate_fixture(
    program_lines: list[str], fallbacks: list[str], *, fail_every_machine: bool
) -> tuple[str, str, str, str]:
    if not program_lines:
        return "error", "", "n-a", ""
    if len(program_lines) == 1 and program_lines[0].strip() == "panicked":
        return "error", "", "n-a", "panicked"

    parsed = []
    for pl in program_lines:
        if pl == "panicked":
            parsed.append({"root": "compilation", "current": "error", "route": "panicked"})
        else:
            parsed.append(field_map(pl, body_only=True))

    current = next((p.get("current", "") for p in parsed if p.get("current")), "")
    if not current:
        current = "error"

    comp = next((p for p in parsed if p.get("root") == "compilation"), parsed[0])
    machine_roots = [p for p in parsed if str(p.get("root", "")).startswith("machine:")]

    if fail_every_machine and comp.get("route") == "no-entry" and machine_roots:
        routes = [p.get("route", "") for p in machine_roots]
        if any(r == "rejected" for r in routes):
            route_verdict = "rejected"
        elif all(r in OUTSIDE_ROUTE or r in ("", "error") for r in routes):
            route_verdict = "outside"
        elif any(r == "accepted" for r in routes) and any(
            r in OUTSIDE_ROUTE for r in routes
        ):
            # One machine judged, another still outside the subset — not fully judged.
            route_verdict = "outside"
        elif any(r == "accepted" for r in routes):
            route_verdict = "accepted"
        else:
            route_verdict = "outside"
    else:
        route_raw = comp.get("route", "error")
        if route_raw in JUDGED_ROUTE:
            route_verdict = route_raw
        elif route_raw in OUTSIDE_ROUTE:
            route_verdict = "outside"
        else:
            route_verdict = "outside"

    first_refusal = first_refusal_from_parsed(parsed, fallbacks)

    if route_verdict == "accepted":
        if current == "accepted":
            agreement = "agree"
        elif current in ("rejected", "stopped"):
            agreement = "disagree"
        else:
            agreement = "n-a"
    elif route_verdict == "rejected":
        if current == "rejected":
            agreement = "agree"
        elif current == "accepted":
            agreement = "disagree"
        else:
            agreement = "n-a"
    elif route_verdict == "outside":
        agreement = "n-a"
    else:
        agreement = "n-a"

    return route_verdict, current, agreement, first_refusal


def load_done() -> set[str]:
    if not TSV.exists():
        return set()
    done: set[str] = set()
    for line in TSV.read_text().splitlines():
        if line.startswith("fixture\t"):
            continue
        if not line.strip():
            continue
        done.add(line.split("\t", 1)[0])
    return done


def run_one(fixture: str) -> tuple[str, float]:
    cmd = [str(HARNESS)]
    if tier(fixture) == "fail":
        cmd.append("--every-machine")
    cmd.append(fixture)
    started = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_SEC,
            stdin=subprocess.DEVNULL,
            cwd=REPO,
        )
        elapsed = time.monotonic() - started
        raw = proc.stdout
        if proc.returncode != 0 and not raw.strip():
            raw = proc.stderr or f"exit={proc.returncode}"
    except subprocess.TimeoutExpired:
        elapsed = float(TIMEOUT_SEC)
        return (
            "\t".join(
                [
                    fixture,
                    tier(fixture),
                    "timeout",
                    "",
                    "n-a",
                    "",
                    f"{elapsed:.1f}",
                ]
            ),
            elapsed,
        )
    except Exception as exc:  # noqa: BLE001
        elapsed = time.monotonic() - started
        return (
            "\t".join(
                [
                    fixture,
                    tier(fixture),
                    "error",
                    "",
                    "n-a",
                    str(exc)[:200],
                    f"{elapsed:.1f}",
                ]
            ),
            elapsed,
        )

    program_lines: list[str] = []
    fallbacks: list[str] = []
    for line in raw.splitlines():
        if line.startswith("program\t"):
            parts = line.split("\t")
            if len(parts) >= 3:
                program_lines.append("\t".join(parts[2:]))
        elif line.startswith("fallback\t"):
            fallbacks.append(line)

    if not program_lines and "panicked" in raw:
        program_lines = ["panicked"]

    route_verdict, current, agreement, first_refusal = aggregate_fixture(
        program_lines, fallbacks, fail_every_machine=tier(fixture) == "fail"
    )
    row = "\t".join(
        [
            fixture,
            tier(fixture),
            route_verdict,
            current,
            agreement,
            first_refusal,
            f"{elapsed:.1f}",
        ]
    )
    return row, elapsed


def repair_all(fixtures: list[str]) -> None:
    header = "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n"
    rows: list[str] = [header.rstrip("\n")]
    for i, fixture in enumerate(fixtures, 1):
        row, elapsed = run_one(fixture)
        rows.append(row)
        if i % 50 == 0 or elapsed > 60:
            print(f"[repair {i}/{len(fixtures)}] {fixture} {elapsed:.1f}s", flush=True)
    TSV.write_text("\n".join(rows) + "\n")


def main() -> None:
    if not HARNESS.is_file():
        print(f"missing harness: {HARNESS}", file=sys.stderr)
        sys.exit(1)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    fixtures = list_shard_fixtures()
    if len(sys.argv) > 2 and sys.argv[2] == "--repair":
        to_repair = fixtures
        existing = load_done()
        if existing:
            to_repair = [f for f in fixtures if f in existing]
        print(f"repair {len(to_repair)} fixtures", flush=True)
        repair_all(to_repair)
        return

    done = load_done()
    todo = [f for f in fixtures if f not in done]
    print(f"shard {SHARD}/{SHARD_MOD}: {len(fixtures)} fixtures, {len(todo)} todo", flush=True)

    if not TSV.exists() or TSV.stat().st_size == 0:
        header = "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n"
        TSV.write_text(header)

    for i, fixture in enumerate(todo, 1):
        row, elapsed = run_one(fixture)
        with TSV.open("a") as f:
            f.write(row + "\n")
        if i % 25 == 0 or elapsed > 60:
            print(f"[{i}/{len(todo)}] {fixture} {elapsed:.1f}s", flush=True)


if __name__ == "__main__":
    main()
