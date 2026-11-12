"""Parse route_one_comparison output into shard TSV fields."""

from __future__ import annotations

META_NO_ENTRY = "meta:no-bound-entry"
META_UNREACHED = "meta:typed-frontier-stopped"

_ENUM_PREFIXES = (
    "Unsupported",
    "Unproved",
    "Unestablished",
    "NotConserved",
    "Conflict",
    "ForeignIdentity",
)

OUTSIDE_FAMILY = frozenset(
    {"outside", "incomplete", "fallback", "refused-module"}
)
UNJUDGED = frozenset({"no-entry", "unreached"})
HARN_ERROR = frozenset({"defect", "panicked"})


def field_map(parts: list[str]) -> dict[str, str]:
    out: dict[str, str] = {}
    i = 0
    while i < len(parts):
        part = parts[i]
        if part == "route=outside":
            out["route"] = "outside"
            if i + 1 < len(parts):
                out["outside_detail"] = parts[i + 1]
                i += 1
            i += 1
            continue
        if "=" in part:
            k, _, v = part.partition("=")
            out[k] = v
        i += 1
    return out


def refusal_feature(detail: str) -> str:
    """Specific construct/feature string, not the generic RefusalReason label."""
    detail = " ".join(detail.split())
    if not detail:
        return "-"
    for prefix in _ENUM_PREFIXES:
        head = f"{prefix}("
        if detail.startswith(head):
            return detail
        if detail.startswith(prefix + ":"):
            rest = detail[len(prefix) + 1 :].strip()
            return rest if rest else detail
        if detail.startswith(prefix + " "):
            return detail[len(prefix) + 1 :].strip() or detail
    return detail


def fixture_route_verdict(routes: list[str]) -> str:
    """Fixture-level route bucket: accepted only if every root accepted and none partial."""
    if not routes:
        return "error"
    if any(r == "rejected" for r in routes):
        return "rejected"
    if any(r in HARN_ERROR for r in routes):
        return "error"

    has_accepted = any(r == "accepted" for r in routes)
    has_outside = any(r in OUTSIDE_FAMILY for r in routes)
    has_unjudged = any(r in UNJUDGED for r in routes)

    if has_accepted and (has_outside or has_unjudged):
        return "partial"
    if has_accepted and all(r == "accepted" for r in routes):
        return "accepted"
    if has_outside or has_unjudged:
        return "outside"
    return "error"


def first_construct_refusal(
    program_fields: list[dict[str, str]], text: str, route_verdict: str
) -> str:
    """First construct-specific refusal in root order; meta bucket when not a construct."""
    for f in program_fields:
        route = f.get("route", "-")
        if route == "outside":
            return refusal_feature(f.get("outside_detail", ""))
        if route == "incomplete":
            for line in text.splitlines():
                if line.startswith("fallback\t"):
                    parts = line.split("\t")
                    if len(parts) >= 6:
                        return refusal_feature(parts[5])
            return "incomplete:unproved-goal-form"

    for line in text.splitlines():
        if line.startswith("fallback\t"):
            parts = line.split("\t")
            if len(parts) >= 5:
                construct = parts[4].strip()
                tail = parts[5].strip() if len(parts) >= 6 else ""
                if tail:
                    return f"{construct}: {refusal_feature(tail)}"
                return construct

    routes = [f.get("route", "-") for f in program_fields]
    if route_verdict == "outside" and all(
        r in UNJUDGED for r in routes
    ):
        return META_NO_ENTRY
    if any(r == "unreached" for r in routes):
        return META_UNREACHED
    if any(r in HARN_ERROR for r in routes):
        return "meta:harness-error"

    for f in program_fields:
        if f.get("route") == "refused-module":
            return refusal_feature(f.get("reason", "refused-module"))

    if route_verdict == "partial":
        for f in program_fields:
            if f.get("route") in OUTSIDE_FAMILY:
                return refusal_feature(f.get("outside_detail", "partial-mix"))
            if f.get("route") in UNJUDGED:
                return META_NO_ENTRY if f.get("route") == "no-entry" else META_UNREACHED

    return "-"


def first_checker_diagnostic_class(text: str) -> str:
    """Classify the first current-checker diagnostic (typed-frontier check only)."""
    for line in text.splitlines():
        if not line.startswith("current-diagnostic\t"):
            continue
        parts = line.split("\t")
        if len(parts) < 4:
            continue
        msg = parts[3].lower()
        if "cannot prove ensures" in msg or "ensures contract" in msg:
            return "ensures"
        if "requires" in msg or "requirement" in msg:
            return "requires"
        if "cannot prove" in msg and "guard" in msg:
            return "guard"
        if "borrow" in msg or "loan" in msg:
            return "borrow"
        if "index" in msg or "bounds" in msg or "bound" in msg:
            return "bounds"
        if "termination" in msg or "well-founded" in msg or "rank" in msg:
            return "termination"
        if "type" in msg and ("mismatch" in msg or "expected" in msg):
            return "type"
        if "build" in msg or "entry" in msg or "program entry" in msg:
            return "build-entry"
        return "other:" + msg[:60].replace("\t", " ")
    return "no-diagnostic"


def parse_harness_output(text: str) -> tuple[str, str, str, str, str]:
    program_fields: list[dict[str, str]] = []
    current = "-"

    for line in text.splitlines():
        if not line.strip():
            continue
        parts = line.split("\t")
        kind = parts[0]
        if kind == "program" and len(parts) >= 3:
            f = field_map(parts[2:])
            if "current" in f and current == "-":
                current = f["current"]
            program_fields.append(f)

    routes = [f.get("route", "-") for f in program_fields]
    route_verdict = fixture_route_verdict(routes)
    first_refusal = "-"
    if route_verdict in ("outside", "partial", "error"):
        first_refusal = first_construct_refusal(program_fields, text, route_verdict)

    agreement = "n-a"
    if current in ("accepted", "rejected") and route_verdict in (
        "accepted",
        "rejected",
    ):
        agreement = "agree" if current == route_verdict else "disagree"
    elif current == "stopped":
        agreement = "n-a"

    diag_class = first_checker_diagnostic_class(text) if current == "rejected" else "-"
    return route_verdict, current, agreement, first_refusal, diag_class
