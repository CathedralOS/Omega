#!/usr/bin/env python3
"""Parse route_one_comparison output into one coverage-map TSV row."""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


@dataclass
class ProgramRoot:
    root: str
    current: str
    route: str
    outside_detail: str | None
    fallback_construct: str | None


def tier_from_path(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


def parse_fields(line: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for part in line.split("\t"):
        if "=" in part:
            key, value = part.split("=", 1)
            out[key] = value
        elif part.startswith("route=outside") or part == "route=outside":
            out["route"] = "outside"
        elif "route" not in out and part in {
            "accepted",
            "rejected",
            "incomplete",
            "fallback",
            "defect",
            "no-entry",
            "unreached",
            "panicked",
        }:
            out["route"] = part
    return out


def parse_outside_detail(fields: list[str]) -> str | None:
    for idx, part in enumerate(fields):
        if part == "route=outside" or part.startswith("route=outside"):
            if idx + 1 < len(fields):
                detail = fields[idx + 1]
                if ":" in detail:
                    return detail.split(":", 1)[1].strip()
                return detail.strip()
            if part.startswith("route=outside\t"):
                pass
        if part.startswith("Unsupported:"):
            return part.split(":", 1)[1].strip()
        if part.startswith("Unestablished:"):
            return part.split(":", 1)[1].strip()
    route = None
    for part in fields:
        if part.startswith("route=outside:"):
            return part.split(":", 1)[1].strip()
        if part.startswith("route="):
            route = part[6:]
    if route and route != "outside" and route.startswith("outside:"):
        return route.split(":", 1)[1].strip()
    return None


def parse_program_line(line: str) -> ProgramRoot | None:
    if not line.startswith("program\t"):
        return None
    fields = line.rstrip("\n").split("\t")
    if len(fields) < 3:
        return None
    kv = parse_fields(line)
    current = kv.get("current", "unknown")
    root = kv.get("root", "compilation")
    route = kv.get("route", "unknown")
    outside_detail = None
    if route == "outside":
        outside_detail = parse_outside_detail(fields[2:])
    fallback_construct = None
    return ProgramRoot(
        root=root,
        current=current,
        route=route,
        outside_detail=outside_detail,
        fallback_construct=fallback_construct,
    )


def parse_fallback_lines(lines: Iterable[str], fixture: str) -> dict[str, str]:
    by_root: dict[str, str] = {}
    prefix = f"fallback\t{fixture}\t"
    for line in lines:
        if not line.startswith(prefix):
            continue
        rest = line[len(prefix) :]
        root_match = re.match(r"root=([^\t]+)\t(\w+)\t", rest)
        if root_match:
            by_root[root_match.group(1)] = root_match.group(2)
    return by_root


def aggregate_route(roots: list[ProgramRoot]) -> tuple[str, str | None]:
    if not roots:
        return "error", None

    routes = [r.route for r in roots]
    first_refusal: str | None = None
    for root in roots:
        if root.route == "outside" and root.outside_detail:
            first_refusal = root.outside_detail
            break

    if any(r in {"rejected", "incomplete"} for r in routes):
        return "rejected", first_refusal
    for undecided in ("fallback", "refused-module", "defect", "panicked"):
        if undecided in routes:
            if first_refusal is None:
                first_refusal = undecided
            return "outside", first_refusal
    if any(r == "accepted" for r in routes):
        if any(r == "outside" for r in routes):
            return "accepted", first_refusal
        if all(r in {"accepted", "no-entry"} for r in routes):
            if routes == ["no-entry"] or all(r == "no-entry" for r in routes):
                return "outside", first_refusal or "no program entry"
            return "accepted", first_refusal
        return "accepted", first_refusal
    if any(r == "outside" for r in routes):
        return "outside", first_refusal
    if any(r == "unreached" for r in routes):
        return "error", first_refusal
    if any(r == "no-entry" for r in routes):
        return "outside", first_refusal or "no program entry"
    return "error", first_refusal


def current_verdict(roots: list[ProgramRoot]) -> str:
    if not roots:
        return "unknown"
    current = roots[0].current
    if current in {"accepted", "rejected", "stopped"}:
        return current
    return current


def agreement(current: str, route: str) -> str:
    if route in {"timeout", "error"} or current == "stopped":
        return "n-a"
    if route == "outside":
        return "n-a"
    if current == "accepted" and route == "accepted":
        return "agree"
    if current == "rejected" and route == "rejected":
        return "agree"
    if current in {"accepted", "rejected"} and route in {"accepted", "rejected"}:
        return "disagree"
    return "n-a"


def row_from_harness_output(
    fixture: str, harness_stdout: str, seconds: float
) -> tuple[str, ...]:
    lines = harness_stdout.splitlines()
    roots: list[ProgramRoot] = []
    fallbacks = parse_fallback_lines(lines, fixture)
    for line in lines:
        parsed = parse_program_line(line)
        if parsed:
            fb = fallbacks.get(parsed.root)
            if fb:
                parsed.fallback_construct = fb
            roots.append(parsed)

    route_verdict, first_refusal = aggregate_route(roots)
    if route_verdict == "outside" and not first_refusal:
        for root in roots:
            if root.fallback_construct:
                first_refusal = root.fallback_construct
                break

    cur = current_verdict(roots)
    agree = agreement(cur, route_verdict)
    tier = tier_from_path(fixture)
    return (
        fixture,
        tier,
        route_verdict,
        cur,
        agree,
        first_refusal or "",
        f"{seconds:.3f}",
    )


def main() -> None:
    if len(sys.argv) != 4:
        print("usage: run_shard_fixture.py <fixture> <harness-stdout-file> <seconds>", file=sys.stderr)
        raise SystemExit(2)
    fixture = sys.argv[1]
    text = Path(sys.argv[2]).read_text(encoding="utf-8", errors="replace")
    seconds = float(sys.argv[3])
    print("\t".join(row_from_harness_output(fixture, text, seconds)))


if __name__ == "__main__":
    main()
