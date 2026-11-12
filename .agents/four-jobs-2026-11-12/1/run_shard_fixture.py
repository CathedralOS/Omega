#!/usr/bin/env python3
"""Run route_one_comparison on one corpus fixture; emit one TSV row."""

from __future__ import annotations

import re
import subprocess
import sys
import time
from pathlib import Path


HARNESS = Path(__file__).resolve().parents[3] / "target/debug/examples/route_one_comparison"
TIMEOUT_SEC = 300


def tier(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


def parse_output(text: str, fixture: str) -> tuple[str, str, str, str]:
    """Return route_verdict, current_verdict, agreement, first_refusal."""
    routes: list[str] = []
    currents: list[str] = []
    first_refusal = ""

    for line in text.splitlines():
        if not line.strip():
            continue
        fields = line.split("\t")
        kind = fields[0] if fields else ""
        if kind == "program" and len(fields) > 1 and fields[1] == fixture:
            route = "-"
            current = "-"
            for f in fields:
                if f.startswith("route="):
                    route = f[len("route=") :]
                elif f.startswith("current="):
                    current = f[len("current=") :]
            if route != "-":
                routes.append(route)
            if current != "-":
                currents.append(current)
            if route.startswith("outside") and not first_refusal:
                # route=outside\t<reason...> may be split across fields after route=
                idx = next((i for i, f in enumerate(fields) if f == "route=outside"), None)
                if idx is not None and idx + 1 < len(fields):
                    first_refusal = fields[idx + 1].split(":")[0].strip()
                else:
                    first_refusal = "outside"
        elif kind == "fallback" and len(fields) > 1 and fields[1] == fixture and not first_refusal:
            if len(fields) >= 5:
                first_refusal = fields[4].strip()
            elif len(fields) >= 4:
                first_refusal = fields[3].strip()

    current_verdict = currents[0] if currents else "unknown"
    if not routes:
        return "error", current_verdict, "n-a", "no-program-line"

    # Aggregate like route_one_comparison --tally (decided before undecided).
    def classify_route(route: str) -> str:
        base = route.split("\t")[0].split()[0]
        if base in ("accepted",):
            return "accepted"
        if base in ("rejected", "incomplete"):
            return "rejected"
        if base.startswith("outside"):
            return "outside"
        if base in (
            "fallback",
            "refused-module",
            "defect",
            "panicked",
            "no-entry",
            "unreached",
        ):
            return "outside"
        return "outside"

    has_rejected = any(classify_route(r) == "rejected" for r in routes)
    has_outside = any(classify_route(r) == "outside" for r in routes)
    has_accepted = any(classify_route(r) == "accepted" for r in routes)

    if has_rejected:
        route_verdict = "rejected"
    elif has_accepted and has_outside:
        route_verdict = "accepted"
    elif has_outside:
        route_verdict = "outside"
    elif has_accepted:
        route_verdict = "accepted"
    else:
        route_verdict = "outside"

    if not first_refusal and route_verdict == "outside":
        for r in routes:
            base = r.split("\t")[0].split()[0]
            if base == "no-entry":
                first_refusal = "no-entry"
                break
            if base == "unreached":
                first_refusal = "unreached"
                break
            if base == "fallback":
                first_refusal = "fallback"
                break
            if base.startswith("outside"):
                m = re.match(r"outside[:\s]+(\S+)", r)
                first_refusal = m.group(1) if m else "outside"
                break
            if base == "refused-module":
                first_refusal = "refused-module"
                break
            if base in ("defect", "panicked", "incomplete"):
                first_refusal = base
                break
        if not first_refusal:
            first_refusal = routes[0].split("\t")[0][:120]

    agreement = "n-a"
    if route_verdict == "accepted" and current_verdict == "accepted":
        agreement = "agree"
    elif route_verdict == "rejected" and current_verdict in ("rejected", "stopped"):
        agreement = "agree"
    elif route_verdict in ("accepted", "rejected") and current_verdict in (
        "accepted",
        "rejected",
        "stopped",
    ):
        agreement = "disagree"

    return route_verdict, current_verdict, agreement, first_refusal


def main() -> None:
    if len(sys.argv) != 2:
        print("usage: run_shard_fixture.py <fixture-path>", file=sys.stderr)
        sys.exit(2)
    fixture = sys.argv[1]
    cmd = [str(HARNESS)]
    if tier(fixture) == "fail":
        cmd.append("--every-machine")
    cmd.append(fixture)
    t0 = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_SEC,
            stdin=subprocess.DEVNULL,
        )
        elapsed = time.perf_counter() - t0
        text = proc.stdout
        if proc.returncode != 0 and not text.strip():
            text = proc.stderr + proc.stdout
        if "panicked" in text and "program\t" not in text:
            row = (
                fixture,
                tier(fixture),
                "error",
                "unknown",
                "n-a",
                "panicked",
                f"{elapsed:.3f}",
            )
            print("\t".join(row))
            return
        route_v, cur_v, agree, refusal = parse_output(text, fixture)
        if route_v == "error":
            row = (fixture, tier(fixture), "error", cur_v, agree, refusal, f"{elapsed:.3f}")
        else:
            row = (fixture, tier(fixture), route_v, cur_v, agree, refusal, f"{elapsed:.3f}")
        print("\t".join(row))
    except subprocess.TimeoutExpired:
        elapsed = time.perf_counter() - t0
        print(
            "\t".join(
                (
                    fixture,
                    tier(fixture),
                    "timeout",
                    "unknown",
                    "n-a",
                    "",
                    f"{elapsed:.3f}",
                )
            )
        )


if __name__ == "__main__":
    main()
