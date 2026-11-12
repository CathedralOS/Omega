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

SKIPPED_ROUTES = frozenset(
    {
        "outside",
        "no-entry",
        "incomplete",
        "fallback",
        "refused-module",
        "defect",
        "panicked",
        "unknown",
    }
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
    return ProgramRoot(
        root=kv.get("root", "compilation"),
        current=kv.get("current", "unknown"),
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
        return rest.split("\t", 1)[0].strip()
    return None


def parse_current_diagnostics(lines: Iterable[str], fixture: str) -> list[str]:
    prefix = f"current-diagnostic\t{fixture}\t"
    messages: list[str] = []
    for line in lines:
        if not line.startswith(prefix):
            continue
        rest = line.split("\t", 3)
        if len(rest) >= 4:
            messages.append(rest[3])
    return messages


def diagnostic_class(message: str) -> str:
    lower = message.lower()
    if "service reach" in lower:
        return "service-reach"
    if "program entry" in lower or "build" in lower and "entry" in lower:
        return "build-entry"
    if "ensures" in lower or "requires" in lower or "proof fact" in lower:
        return "contract"
    if "borrow" in lower or "loan" in lower or "&mut" in lower or "mutable" in lower:
        return "borrow"
    if "division" in lower or "divide" in lower or "divisor" in lower:
        return "division"
    if "assembly" in lower or "instruction" in lower or "clobber" in lower:
        return "asm"
    if "cannot prove" in lower or "unproved" in lower:
        return "unproved-obligation"
    return "other"


def machine_from_root(root: str) -> str | None:
    if root.startswith("machine:"):
        return root[8:]
    return None


def machines_named_in_diagnostics(messages: list[str]) -> set[str]:
    found: set[str] = set()
    for message in messages:
        for match in re.finditer(r"machine `([^`]+)`", message):
            found.add(match.group(1))
    return found


def first_outside_construct(roots: list[ProgramRoot]) -> str | None:
    for root in roots:
        if root.route == "outside" and root.construct:
            return root.construct
        if root.route == "outside":
            return "outside-without-detail"
    return None


def fixture_verdict(
    fixture: str, roots: list[ProgramRoot], diagnostics: list[str]
) -> tuple[str, str | None]:
    if not roots:
        return "error", BUCKET_STOPPED

    current = roots[0].current
    first_construct = first_outside_construct(roots)

    if any(r.route == "unreached" for r in roots):
        extra = next((r.extra for r in roots if r.route == "unreached" and r.extra), None)
        bucket = BUCKET_STOPPED
        if extra:
            text = extra.strip()
            if text.startswith("Some(") and text.endswith(")"):
                text = text[5:-1].strip().strip('"')
            bucket = f"{BUCKET_STOPPED}:{text[:160]}"
        return "error", bucket

    if current == "stopped":
        return "error", BUCKET_STOPPED

    routes = [r.route for r in roots]

    if any(r == "rejected" for r in routes):
        return "rejected", first_construct if current == "rejected" else None

    accepted_roots = [r for r in roots if r.route == "accepted"]
    skipped_roots = [r for r in roots if r.route in SKIPPED_ROUTES]

    if current == "rejected":
        # Fail/ multi-root: route "accepted" only if every rejected part was judged.
        if accepted_roots and skipped_roots:
            return "partial", BUCKET_PARTIAL
        if len(roots) == 1:
            only = roots[0]
            if only.route == "accepted":
                return "accepted", None
            if only.route == "no-entry":
                return "no-entry", BUCKET_NO_ENTRY
            if only.route == "outside":
                return "outside", only.construct or first_construct
        if accepted_roots and not skipped_roots:
            return "accepted", None
        if all(r.route == "no-entry" for r in roots):
            return "no-entry", BUCKET_NO_ENTRY
        if any(r.route == "outside" for r in roots):
            return "outside", first_construct
        if any(r.route == "incomplete" for r in routes):
            return "outside", first_construct or BUCKET_INCOMPLETE
        return "outside", first_construct

    # current == accepted
    if any(r == "rejected" for r in routes):
        return "rejected", None
    if any(r == "accepted" for r in routes):
        return "accepted", None
    if all(r == "no-entry" for r in routes):
        return "no-entry", BUCKET_NO_ENTRY
    if any(r == "outside" for r in routes):
        return "outside", first_construct
    if any(r == "no-entry" for r in routes):
        return "no-entry", BUCKET_NO_ENTRY
    return "outside", first_construct


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

    diagnostics = parse_current_diagnostics(lines, fixture)
    route_verdict, first_refusal = fixture_verdict(fixture, roots, diagnostics)

    if route_verdict == "partial":
        first_refusal = BUCKET_PARTIAL
    elif route_verdict not in {"outside", "error", "no-entry"}:
        first_refusal = None
    elif route_verdict == "no-entry":
        first_refusal = BUCKET_NO_ENTRY

    cur = roots[0].current if roots else "unknown"
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
