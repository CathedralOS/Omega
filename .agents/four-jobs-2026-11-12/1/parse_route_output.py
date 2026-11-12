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


def combined_route(routes: list[str]) -> str:
    """Mirror route_one_comparison --tally Tallied::route over machine roots."""
    if not routes:
        return "error"

    has = lambda name: any(r == name or r.startswith(name + ",") for r in routes)
    first = lambda name: next((r for r in routes if r == name or r.startswith(name)), None)

    for decided in ("rejected", "incomplete"):
        if has(decided):
            return decided
    for undecided in ("fallback", "refused-module", "defect", "panicked"):
        found = first(undecided)
        if found:
            return found
    if has("accepted"):
        return "accepted"
    if first("outside") or any(r == "outside" for r in routes):
        return "outside"
    if has("no-entry"):
        return "no-entry"
    if has("unreached"):
        return "unreached"
    return routes[0]


def bucket_route(combined: str) -> str:
    if combined == "rejected":
        return "rejected"
    if combined == "accepted":
        return "accepted"
    if combined in (
        "incomplete",
        "fallback",
        "refused-module",
        "outside",
        "no-entry",
    ):
        return "outside"
    if combined in ("defect", "panicked", "unreached", "error"):
        return "error"
    return "error"


def first_construct_refusal(
    program_fields: list[dict[str, str]], text: str, combined: str
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

    if combined in ("no-entry",) or all(
        f.get("route") in ("no-entry", "-") for f in program_fields
    ):
        return META_NO_ENTRY
    if combined == "unreached" or any(f.get("route") == "unreached" for f in program_fields):
        return META_UNREACHED
    if combined in ("defect", "panicked"):
        return f"meta:harness-{combined}"

    for f in program_fields:
        if f.get("route") == "refused-module":
            return refusal_feature(f.get("reason", "refused-module"))

    return "-"


def parse_harness_output(text: str) -> tuple[str, str, str, str]:
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
    combined = combined_route(routes)
    route_verdict = bucket_route(combined)
    first_refusal = "-"
    if route_verdict == "outside":
        first_refusal = first_construct_refusal(program_fields, text, combined)
    elif route_verdict == "error":
        first_refusal = first_construct_refusal(program_fields, text, combined)

    agreement = "n-a"
    if current in ("accepted", "rejected") and route_verdict in ("accepted", "rejected"):
        agreement = "agree" if current == route_verdict else "disagree"
    elif current == "stopped":
        agreement = "n-a"

    return route_verdict, current, agreement, first_refusal
