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


def root_column_label(fields: dict[str, str]) -> str:
    root = fields.get("root", "compilation")
    if root == "compilation":
        return "compilation"
    if root.startswith("machine:"):
        return root
    return root


def normalize_root_verdict(route: str) -> str:
    if route in ("accepted", "rejected", "outside", "no-entry"):
        return route
    if route in OUTSIDE_FAMILY:
        return "outside"
    if route == "no-entry":
        return "no-entry"
    if route in UNJUDGED:
        return "no-entry" if route == "no-entry" else "error"
    if route in HARN_ERROR:
        return "error"
    return "error"


def program_verdict(root_verdicts: list[str]) -> str:
    if not root_verdicts:
        return "error"
    if any(v == "rejected" for v in root_verdicts):
        return "rejected"
    if any(v == "error" for v in root_verdicts) and not any(
        v == "accepted" for v in root_verdicts
    ):
        return "error"

    has_accepted = any(v == "accepted" for v in root_verdicts)
    has_judged = any(v in ("accepted", "rejected") for v in root_verdicts)
    has_non_judged = any(
        v in ("outside", "no-entry", "error") for v in root_verdicts
    )

    if has_accepted and has_non_judged:
        return "partial"
    if has_accepted and all(v == "accepted" for v in root_verdicts):
        return "accepted"
    if not has_judged:
        return "outside"
    return "outside"


def first_construct_refusal(
    program_fields: list[dict[str, str]], text: str, route_verdict: str
) -> str:
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
        normalize_root_verdict(r) == "no-entry" for r in routes
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
            if normalize_root_verdict(f.get("route", "-")) == "no-entry":
                return META_NO_ENTRY

    return "-"


def first_checker_diagnostic_class(text: str) -> str:
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


def parse_program_roots(
    program_fields: list[dict[str, str]],
) -> tuple[str, str, list[str]]:
    parts: list[str] = []
    vacuous: list[str] = []
    norm_verdicts: list[str] = []

    for f in program_fields:
        label = root_column_label(f)
        raw_route = f.get("route", "error")
        verdict = normalize_root_verdict(raw_route)
        parts.append(f"{label}:{verdict}")
        norm_verdicts.append(verdict)

        if verdict == "accepted":
            try:
                obligations = int(f.get("obligations", "-1"))
                judged = int(f.get("judged", "-1"))
            except ValueError:
                obligations = judged = -1
            if obligations == 0 and judged == 0:
                vacuous.append(label)

    roots = ";".join(parts) if parts else "-"
    vacuous_str = ";".join(vacuous) if vacuous else ""
    return roots, vacuous_str, norm_verdicts


def parse_harness_output(
    text: str,
) -> tuple[str, str, str, str, str, str, str]:
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

    roots, vacuous, norm_verdicts = parse_program_roots(program_fields)
    route_verdict = program_verdict(norm_verdicts)

    first_refusal = "-"
    if route_verdict in ("outside", "partial"):
        first_refusal = first_construct_refusal(program_fields, text, route_verdict)
    elif route_verdict == "error":
        first_refusal = META_UNREACHED if any(
            f.get("route") == "unreached" for f in program_fields
        ) else "meta:harness-error"
    elif route_verdict == "rejected":
        first_refusal = "-"

    agreement = "n-a"
    if current in ("accepted", "rejected") and route_verdict in (
        "accepted",
        "rejected",
    ):
        agreement = "agree" if current == route_verdict else "disagree"
    elif current == "stopped":
        agreement = "n-a"

    diag_class = first_checker_diagnostic_class(text) if current == "rejected" else "-"
    return route_verdict, current, agreement, first_refusal, diag_class, roots, vacuous
