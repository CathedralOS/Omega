#!/usr/bin/env python3
"""Run route_one_comparison on one corpus fixture; emit one TSV row."""

from __future__ import annotations

import re
import subprocess
import sys
import time
from dataclasses import dataclass
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


@dataclass
class ProgramRoot:
    root: str
    current: str
    route: str
    refusal_detail: str
    fields: list[str]


def field_value(fields: list[str], prefix: str) -> str:
    for f in fields:
        if f.startswith(prefix):
            return f[len(prefix) :]
    return ""


def refusal_construct(route: str, detail: str, fields: list[str]) -> str:
    """Map a route refusal to the construct or meta-bucket for ranking."""
    if route == "no-entry":
        return "meta:no-program-entry"
    if route == "unreached":
        return "meta:typed-frontier-stopped"
    if route == "refused-module":
        reason = field_value(fields, "reason=")
        return reason if reason else "refused-module"
    if route == "fallback":
        # fallback lines use field 4 for Concerns debug; detail in field 5
        return detail if detail else "fallback"
    if detail:
        # e.g. "Unsupported: machine generic parameters"
        if ": " in detail:
            _kind, spec = detail.split(": ", 1)
            return spec.strip() or detail.strip()
        return detail.strip()
    if route in ("defect", "panicked", "incomplete"):
        return route
    return route if route else "outside"


def classify_route(route: str) -> str:
    base = route.split()[0]
    if base == "accepted":
        return "accepted"
    if base in ("rejected", "incomplete"):
        return "rejected"
    if base == "outside" or base.startswith("outside"):
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


def parse_program_lines(text: str, fixture: str) -> tuple[list[ProgramRoot], list[tuple[str, str, str]]]:
    """Return program roots in harness order and fallback rows (root, construct, detail)."""
    programs: list[ProgramRoot] = []
    fallbacks: list[tuple[str, str, str]] = []
    for line in text.splitlines():
        if not line.strip():
            continue
        fields = line.split("\t")
        kind = fields[0]
        if kind == "program" and len(fields) > 1 and fields[1] == fixture:
            current = field_value(fields, "current=")
            root = field_value(fields, "root=")
            route = ""
            detail = ""
            for i, f in enumerate(fields):
                if f == "route=outside" and i + 1 < len(fields):
                    route = "outside"
                    detail = fields[i + 1]
                    break
                if f.startswith("route="):
                    route = f[len("route=") :]
            programs.append(
                ProgramRoot(
                    root=root,
                    current=current,
                    route=route,
                    refusal_detail=detail,
                    fields=fields,
                )
            )
        elif kind == "fallback" and len(fields) > 1 and fields[1] == fixture:
            root = field_value(fields, "root=")
            construct = fields[4].strip() if len(fields) > 4 else ""
            detail = fields[5].strip() if len(fields) > 5 else ""
            fallbacks.append((root, construct, detail))
    return programs, fallbacks


def aggregate_route(programs: list[ProgramRoot]) -> str:
    routes = [p.route for p in programs if p.route]
    if not routes:
        return "error"
    has_rejected = any(classify_route(r) == "rejected" for r in routes)
    has_outside = any(classify_route(r) == "outside" for r in routes)
    has_accepted = any(classify_route(r) == "accepted" for r in routes)
    if has_rejected:
        return "rejected"
    if has_accepted and has_outside:
        return "accepted"
    if has_outside:
        return "outside"
    if has_accepted:
        return "accepted"
    return "outside"


def first_refusal(
    programs: list[ProgramRoot],
    fallbacks: list[tuple[str, str, str]],
    fixture: str,
) -> str:
    skip_entry_no_entry = tier(fixture) == "fail" and any(
        p.root.startswith("machine:") for p in programs
    )
    fallback_by_root = {r: (c, d) for r, c, d in fallbacks}
    for prog in programs:
        if (
            skip_entry_no_entry
            and prog.root == "compilation"
            and prog.route == "no-entry"
        ):
            continue
        base = prog.route.split()[0] if prog.route else ""
        if base in ("outside", "no-entry", "unreached", "fallback", "refused-module"):
            if base == "fallback" and prog.root in fallback_by_root:
                c, d = fallback_by_root[prog.root]
                return refusal_construct("fallback", f"{c}: {d}".strip(": "), prog.fields)
            return refusal_construct(base, prog.refusal_detail, prog.fields)
        if base == "incomplete":
            return "incomplete-proof-rule"
        if base in ("defect", "panicked"):
            return base
    # Fallback lines without a matching program route=fallback in program lines
    for _root, construct, detail in fallbacks:
        return refusal_construct("fallback", f"{construct}: {detail}".strip(": "), [])
    return ""


def parse_output(text: str, fixture: str) -> tuple[str, str, str, str]:
    programs, fallbacks = parse_program_lines(text, fixture)
    if not programs:
        return "error", "unknown", "n-a", "no-program-line"

    current_verdict = programs[0].current or "unknown"
    route_verdict = aggregate_route(programs)
    refusal = first_refusal(programs, fallbacks, fixture)

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
    elif route_verdict == "outside":
        agreement = "n-a"

    return route_verdict, current_verdict, agreement, refusal


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
