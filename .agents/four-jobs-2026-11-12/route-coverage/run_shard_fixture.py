#!/usr/bin/env python3
"""Parse route_one_comparison output into one coverage-map TSV row."""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

BUCKET_NO_ENTRY = "bucket:no-program-entry"
BUCKET_STOPPED = "bucket:typed-frontier-stopped"
BUCKET_MODULE_REFUSED = "bucket:module-refused"
BUCKET_INCOMPLETE = "bucket:prover-incomplete"
BUCKET_FALLBACK = "bucket:checker-fallback"
BUCKET_PARTIAL = "bucket:partial-unjudged-defect"

REFUSAL_PREFIXES = (
    "Unsupported:",
    "Unestablished:",
    "ForeignIdentity:",
    "NotConserved:",
    "Conflict:",
)

ROOT_VERDICTS = frozenset({"accepted", "rejected", "outside", "no-entry", "error", "timeout"})
JUDGED_ROOT = frozenset({"accepted", "rejected"})
UNJUDGED_ROOT = frozenset(
    {
        "outside",
        "no-entry",
        "incomplete",
        "fallback",
        "refused-module",
        "defect",
        "panicked",
        "unknown",
        "error",
    }
)


@dataclass
class ProgramRoot:
    root: str
    current: str
    route: str
    construct: str | None
    extra: str | None
    obligations: int
    judged: int


def tier_from_path(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


def parse_int_fields(parts: list[str]) -> dict[str, int]:
    out: dict[str, int] = {}
    for part in parts:
        for key in ("obligations", "judged"):
            if part.startswith(f"{key}="):
                try:
                    out[key] = int(part.split("=", 1)[1])
                except ValueError:
                    out[key] = 0
    return out


def kv_pairs(parts: list[str]) -> dict[str, str]:
    out: dict[str, str] = {}
    for part in parts:
        if "=" not in part:
            continue
        key, value = part.split("=", 1)
        if key in {"current", "root", "route", "reason"}:
            out[key] = value
    return out


def extract_construct(detail: str) -> str:
    detail = detail.strip()
    for prefix in REFUSAL_PREFIXES:
        if detail.startswith(prefix):
            return detail.split(":", 1)[1].strip()
    if ":" in detail and detail.split(":", 1)[0] in {
        "Unsupported",
        "Unestablished",
        "ForeignIdentity",
        "NotConserved",
        "Conflict",
    }:
        return detail.split(":", 1)[1].strip()
    return detail


def parse_route_tail(tail: list[str]) -> tuple[str, str | None, str | None]:
    for index, part in enumerate(tail):
        if part == "route=outside":
            if index + 1 < len(tail):
                nxt = tail[index + 1]
                if not nxt.startswith("route_") and "=" not in nxt[:12]:
                    return "outside", extract_construct(nxt), None
            return "outside", None, None
        if part.startswith("route="):
            route = part[6:]
            if route.startswith("outside:"):
                return "outside", extract_construct(route), None
            if route == "unreached":
                extra = tail[index + 1] if index + 1 < len(tail) else None
                return "unreached", BUCKET_STOPPED, extra
            if route == "no-entry":
                return "no-entry", BUCKET_NO_ENTRY, None
            if route == "refused-module":
                reason = next(
                    (p.split("=", 1)[1] for p in tail[index + 1 :] if p.startswith("reason=")),
                    None,
                )
                construct = f"{BUCKET_MODULE_REFUSED}:{reason}" if reason else BUCKET_MODULE_REFUSED
                return "refused-module", construct, reason
            if route == "incomplete":
                return "incomplete", BUCKET_INCOMPLETE, None
            if route == "fallback":
                return "fallback", BUCKET_FALLBACK, None
            return route, None, None
    return "unknown", None, None


def root_verdict_for_column(route: str) -> str:
    if route == "unreached":
        return "error"
    if route in {"incomplete", "fallback", "refused-module", "defect", "panicked", "unknown"}:
        return "outside"
    if route in ROOT_VERDICTS:
        return route
    return "outside"


def parse_program_line(line: str) -> ProgramRoot | None:
    if not line.startswith("program\t"):
        return None
    parts = line.rstrip("\n").split("\t")
    if len(parts) < 3:
        return None
    kv = kv_pairs(parts[2:])
    route, construct, extra = parse_route_tail(parts[2:])
    counts = parse_int_fields(parts[2:])
    return ProgramRoot(
        root=kv.get("root", "compilation"),
        current=kv.get("current", "unknown"),
        route=route,
        construct=construct,
        extra=extra,
        obligations=counts.get("obligations", 0),
        judged=counts.get("judged", 0),
    )


def parse_fallback_construct(lines: Iterable[str], fixture: str, root: str) -> str | None:
    prefix = f"fallback\t{fixture}\troot={root}\t"
    for line in lines:
        if not line.startswith(prefix):
            continue
        rest = line[len(prefix) :]
        return rest.split("\t", 1)[0].strip()
    return None


def format_roots(roots: list[ProgramRoot]) -> str:
    parts: list[str] = []
    for root in roots:
        label = root.root
        verdict = root_verdict_for_column(root.route)
        parts.append(f"{label}:{verdict}")
    return ";".join(parts)


def vacuous_accept_roots(roots: list[ProgramRoot]) -> str:
    vacuous: list[str] = []
    for root in roots:
        if root.route == "accepted" and root.obligations == 0 and root.judged == 0:
            vacuous.append(root.root)
    return ";".join(vacuous)


def first_outside_construct(roots: list[ProgramRoot]) -> str | None:
    for root in roots:
        if root.route == "outside" and root.construct:
            return root.construct
        if root.route == "outside":
            return "outside-without-detail"
    return None


def error_bucket(roots: list[ProgramRoot]) -> str:
    for root in roots:
        if root.route == "unreached" and root.extra:
            text = root.extra.strip()
            if text.startswith("Some(") and text.endswith(")"):
                text = text[5:-1].strip().strip('"')
            return f"{BUCKET_STOPPED}:{text[:160]}"
    return BUCKET_STOPPED


def program_verdict(roots: list[ProgramRoot], current: str) -> tuple[str, str | None]:
    """Uniform root aggregation for merge."""
    if not roots:
        return "error", BUCKET_STOPPED

    if current == "stopped" or any(r.route == "unreached" for r in roots):
        # Program-level error only when no judged root exists.
        column_routes = [root_verdict_for_column(r.route) for r in roots]
        if not any(v in JUDGED_ROOT for v in column_routes):
            return "error", error_bucket(roots)

    column_routes = [root_verdict_for_column(r.route) for r in roots]
    has_rejected = any(v == "rejected" for v in column_routes)
    has_accepted = any(v == "accepted" for v in column_routes)
    has_unjudged = any(v in {"outside", "no-entry", "error"} for v in column_routes)

    if has_rejected:
        return "rejected", first_outside_construct(roots)

    if has_accepted and not has_unjudged and all(v == "accepted" for v in column_routes):
        return "accepted", None

    if has_accepted and has_unjudged:
        return "partial", BUCKET_PARTIAL

    if has_accepted:
        return "accepted", None

    # Nothing judged at accepted/rejected level.
    if all(v == "no-entry" for v in column_routes):
        return "no-entry", BUCKET_NO_ENTRY
    if all(v == "error" for v in column_routes):
        return "error", error_bucket(roots)
    return "outside", first_outside_construct(roots)


def agreement(current: str, route: str) -> str:
    if route in {"timeout", "error", "partial", "outside", "no-entry"}:
        return "n-a"
    if current == "stopped":
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
    for line in lines:
        parsed = parse_program_line(line)
        if parsed:
            if parsed.route == "fallback" and not parsed.construct:
                fb = parse_fallback_construct(lines, fixture, parsed.root)
                if fb:
                    parsed.construct = f"{BUCKET_FALLBACK}:{fb}"
            roots.append(parsed)

    roots_col = format_roots(roots)
    vacuous_col = vacuous_accept_roots(roots)
    cur = roots[0].current if roots else "unknown"
    route_verdict, first_refusal = program_verdict(roots, cur)

    if route_verdict == "partial":
        first_refusal = BUCKET_PARTIAL
    elif route_verdict == "no-entry":
        first_refusal = BUCKET_NO_ENTRY
    elif route_verdict == "error":
        first_refusal = error_bucket(roots)
    elif route_verdict != "outside":
        first_refusal = None

    agree = agreement(cur, route_verdict)
    tier = tier_from_path(fixture)
    return (
        fixture,
        tier,
        route_verdict,
        cur,
        agree,
        first_refusal or "",
        seconds if isinstance(seconds, str) else f"{float(seconds):.3f}",
        roots_col,
        vacuous_col,
    )


def row_from_timeout(fixture: str, seconds: float, tier: str) -> tuple[str, ...]:
    return (
        fixture,
        tier,
        "timeout",
        "?",
        "n-a",
        "",
        f"{seconds:.3f}",
        "compilation:timeout",
        "",
    )


def row_from_harness_failure(fixture: str, seconds: float, tier: str, detail: str) -> tuple[str, ...]:
    return (
        fixture,
        tier,
        "error",
        "?",
        "n-a",
        f"bucket:harness-failure:{detail[:160]}",
        f"{seconds:.3f}",
        "compilation:error",
        "",
    )


def main() -> None:
    if len(sys.argv) != 4:
        print(
            "usage: run_shard_fixture.py <fixture> <harness-stdout-file> <seconds>",
            file=sys.stderr,
        )
        raise SystemExit(2)
    fixture = sys.argv[1]
    text = Path(sys.argv[2]).read_text(encoding="utf-8", errors="replace")
    seconds = float(sys.argv[3])
    print("\t".join(row_from_harness_output(fixture, text, seconds)))


if __name__ == "__main__":
    main()
