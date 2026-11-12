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

REFUSAL_PREFIXES = (
    "Unsupported:",
    "Unestablished:",
    "ForeignIdentity:",
    "NotConserved:",
    "Conflict:",
)


@dataclass
class ProgramRoot:
    root: str
    current: str
    route: str
    construct: str | None
    extra: str | None


def tier_from_path(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


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
    """Return (route, construct or bucket, extra detail)."""
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


def parse_program_line(line: str) -> ProgramRoot | None:
    if not line.startswith("program\t"):
        return None
    parts = line.rstrip("\n").split("\t")
    if len(parts) < 3:
        return None
    kv = kv_pairs(parts[2:])
    route, construct, extra = parse_route_tail(parts[2:])
    current = kv.get("current", "unknown")
    root = kv.get("root", "compilation")
    return ProgramRoot(
        root=root,
        current=current,
        route=route,
        construct=construct,
        extra=extra,
    )


def parse_fallback_construct(lines: Iterable[str], fixture: str, root: str) -> str | None:
    prefix = f"fallback\t{fixture}\troot={root}\t"
    for line in lines:
        if not line.startswith(prefix):
            continue
        rest = line[len(prefix) :]
        construct = rest.split("\t", 1)[0]
        return construct.strip()
    return None


def aggregate_route(roots: list[ProgramRoot]) -> tuple[str, str | None]:
    if not roots:
        return "error", BUCKET_STOPPED

    routes = [r.route for r in roots]
    first_construct: str | None = None
    for root in roots:
        if root.route == "outside" and root.construct:
            first_construct = root.construct
            break
        if root.route == "outside" and root.construct is None:
            first_construct = "outside-without-detail"
            break

    if any(r == "unreached" for r in roots):
        extra = next((r.extra for r in roots if r.route == "unreached" and r.extra), None)
        bucket = BUCKET_STOPPED
        if extra:
            text = extra.strip()
            if text.startswith("Some(") and text.endswith(")"):
                text = text[5:-1].strip().strip('"')
            bucket = f"{BUCKET_STOPPED}:{text[:160]}"
        return "error", bucket

    if any(r == "rejected" for r in routes):
        return "rejected", first_construct
    if any(r == "incomplete" for r in routes):
        return "outside", first_construct or BUCKET_INCOMPLETE

    for undecided in ("fallback", "refused-module", "defect", "panicked"):
        if undecided in routes:
            for root in roots:
                if root.route == undecided and root.construct:
                    return "outside", root.construct
            return "outside", BUCKET_FALLBACK

    if any(r == "accepted" for r in routes):
        if any(r == "outside" for r in routes):
            return "accepted", None
        only_no_entry = all(r in {"accepted", "no-entry"} for r in routes)
        if only_no_entry and any(r == "no-entry" for r in routes) and not any(
            r == "accepted" for r in routes
        ):
            return "outside", BUCKET_NO_ENTRY
        if only_no_entry and any(r == "accepted" for r in routes):
            return "accepted", None
        return "accepted", None

    if any(r == "outside" for r in routes):
        return "outside", first_construct

    if all(r == "no-entry" for r in routes):
        return "outside", BUCKET_NO_ENTRY

    if any(r == "no-entry" for r in routes) and any(r == "outside" for r in routes):
        return "outside", first_construct

    return "error", BUCKET_STOPPED


def current_verdict(roots: list[ProgramRoot]) -> str:
    if not roots:
        return "unknown"
    return roots[0].current


def agreement(current: str, route: str) -> str:
    """Route verdict vs harness `current=` only (not corpus tier or ledger)."""
    if route in {"timeout", "error"}:
        return "n-a"
    if current == "stopped":
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
    for line in lines:
        parsed = parse_program_line(line)
        if parsed:
            if parsed.route == "fallback" and not parsed.construct:
                fb = parse_fallback_construct(lines, fixture, parsed.root)
                if fb:
                    parsed.construct = f"{BUCKET_FALLBACK}:{fb}"
            roots.append(parsed)

    route_verdict, first_refusal = aggregate_route(roots)
    if route_verdict not in {"outside", "error"}:
        first_refusal = None

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
