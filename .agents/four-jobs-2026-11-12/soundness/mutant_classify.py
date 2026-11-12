"""Shared disposition rules for soundness mutation shards (lead review 2026-11-12)."""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class SpecRule:
    heading: str
    path: str
    quote: str


SPEC_RULES: dict[str, SpecRule] = {
    "tighten_requires_equality": SpecRule(
        "Machine requirements at invocation",
        "wiki/spec/language/state_contracts.md",
        "Every arrival establishes the target's requirements and declared domains "
        "under actual argument substitution.",
    ),
    "remove_mut_binding": SpecRule(
        "Owned local mutability",
        "wiki/spec/language/ownership.md",
        "Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established "
        "ordinary local's owned storage requires a `mut` binding. This covers direct "
        "assignment, writes to owned fields or array elements, and formation of "
        "`&mut` or `&write` loans to those places.",
    ),
    "demote_mut_ref_to_shared": SpecRule(
        "Borrows and aliases",
        "wiki/spec/language/ownership.md",
        "`&mut T` | Exclusive observation and mutation.",
    ),
    "promote_shared_ref_to_mut": SpecRule(
        "Borrows and aliases",
        "wiki/spec/language/ownership.md",
        "`&T` | Shared observation under its loan contract.",
    ),
    "corrupt_literal_callee_argument": SpecRule(
        "Machine requirements at invocation",
        "wiki/spec/language/state_contracts.md",
        "Machine `requires` applies at every invocation, including terminal and recursive "
        "calls. Every arrival establishes the target's requirements and declared domains "
        "under actual argument substitution.",
    ),
    "delete_transition_guard": SpecRule(
        "Transitions",
        "wiki/spec/language/state_contracts.md",
        "Transition guards establish that only enabled edges may fire.",
    ),
    "drop_requires_line": SpecRule(
        "Machine requirements at invocation",
        "wiki/spec/language/state_contracts.md",
        "Machine `requires` applies at every invocation, including terminal and recursive "
        "calls. Every arrival establishes the target's requirements and declared domains "
        "under actual argument substitution.",
    ),
}

ROUTE_NON_REJECT_REASONS = frozenset(
    {"no-entry", "unreached", "fallback", "incomplete", "defect", "stopped"}
)

BUCKETS = (
    "confirmed-invalid",
    "checker-disagreement",
    "known-gap-q7-mut",
    "discarded-valid",
    "discarded-no-rule",
    "discarded-both-reject",
    "discarded-timeout",
    "discarded-run-failed",
    "discarded-generate-skipped",
)


@dataclass
class ClassifyResult:
    disposition: str
    hand_check: str
    spec: SpecRule | None
    obligation_line: str
    route_reason: str = ""


def _line_evidence(mutant: str, pattern: str) -> str:
    for line in mutant.splitlines():
        if re.search(pattern, line):
            return line.strip()
    return ""


def remove_mut_obligation(original: str, mutant: str) -> tuple[bool, str]:
    bind = None
    for match in re.finditer(r"(?m)^(\s*)let mut ([A-Za-z_][A-Za-z0-9_]*)\b", original):
        name = match.group(2)
        spacing = match.group(1)
        demoted = f"{spacing}let {name}"
        if demoted in mutant and match.group(0) not in mutant:
            bind = name
            break
    if not bind:
        return False, ""

    past_binding = False
    write_line = ""
    for line in mutant.splitlines():
        if re.search(rf"^\s*let\s+{re.escape(bind)}\b", line) and "let mut" not in line:
            past_binding = True
            continue
        if not past_binding:
            continue
        if re.search(rf"^\s*let\s+mut\s+{re.escape(bind)}\b", line):
            return False, ""
        if re.search(rf"^\s*let\s+{re.escape(bind)}\b", line):
            return False, ""
        if re.search(
            rf"\b{re.escape(bind)}\s*=|\b{re.escape(bind)}\[[^\]]*\]\s*=|"
            rf"\b{re.escape(bind)}\.[A-Za-z_0-9]+\s*=|"
            rf"&mut\s+{re.escape(bind)}\b|&write\s+{re.escape(bind)}\b",
            line,
        ):
            write_line = line.strip()
            break
    if not write_line:
        return False, ""
    return True, write_line


def tighten_requires_obligation(original: str, mutant: str) -> tuple[bool, str]:
    orig_req = re.search(r"(?m)^\s*requires\b[^\n]+", original)
    mut_req = re.search(r"(?m)^\s*requires\b[^\n]+", mutant)
    if not orig_req or not mut_req or orig_req.group(0) == mut_req.group(0):
        return False, ""
    call = re.search(r"(?m)^\s*[A-Za-z_][\w:]*\([^)]*\)", mutant)
    if call:
        return True, f"{mut_req.group(0).strip()} vs call `{call.group(0).strip()}`"
    return True, mut_req.group(0).strip()


def _param_names_marked_mut(original: str, mutant: str) -> list[str]:
    changed: list[str] = []
    for line_o, line_m in zip(original.splitlines(), mutant.splitlines()):
        if line_o == line_m:
            continue
        for match in re.finditer(
            r"([A-Za-z_][\w]*)\s*:\s*&mut\b", line_o
        ):
            name = match.group(1)
            if re.search(rf"{re.escape(name)}\s*:\s*&\b", line_m):
                changed.append(name)
    return changed


def _param_names_marked_shared(original: str, mutant: str) -> list[str]:
    changed: list[str] = []
    for line_o, line_m in zip(original.splitlines(), mutant.splitlines()):
        if line_o == line_m:
            continue
        for match in re.finditer(r"([A-Za-z_][\w]*)\s*:\s*&\b", line_o):
            name = match.group(1)
            if re.search(rf"{re.escape(name)}\s*:\s*&mut\b", line_m):
                changed.append(name)
    return changed


def ref_demote_obligation(original: str, mutant: str) -> tuple[bool, str]:
    if "&mut " not in original:
        return False, ""
    for name in _param_names_marked_mut(original, mutant):
        for line in mutant.splitlines():
            if re.search(
                rf"\b{name}\s*\.[A-Za-z_0-9]+\s*=|\b{name}\s*\[[^\]]*\]\s*=|\b{name}\s*=",
                line,
            ):
                return True, line.strip()
    return False, ""


def ref_promote_obligation(original: str, mutant: str) -> tuple[bool, str]:
    for name in _param_names_marked_shared(original, mutant):
        for line in mutant.splitlines():
            if re.search(
                rf"\b{name}\s*\.[A-Za-z_0-9]+\s*=|\b{name}\s*\[[^\]]*\]\s*=|\b{name}\s*=",
                line,
            ):
                return True, line.strip()
    return False, ""


def corrupt_call_obligation(_original: str, mutant: str) -> tuple[bool, str]:
    call = re.search(r"(?m)^\s*[A-Za-z_][\w:]*\([^)]*0[^)]*\)", mutant)
    if call:
        return True, call.group(0).strip()
    return False, ""


def drop_requires_removed_line(original: str, mutant: str) -> str:
    for line in original.splitlines():
        stripped = line.strip()
        if not stripped.startswith("requires"):
            continue
        if stripped not in {item.strip() for item in mutant.splitlines()}:
            return stripped
    return ""


def requires_fact_repeated_elsewhere(removed: str, mutant: str) -> tuple[bool, str]:
    """True when transition/ensures restates the same fact (with quoted lines)."""
    tokens = re.findall(r"[A-Za-z_][\w]*|\d+", removed)
    if not tokens:
        return False, ""
    for line in mutant.splitlines():
        if not (
            line.strip().startswith("transition")
            or line.strip().startswith("ensures")
            or "->" in line
        ):
            continue
        if all(token in line for token in tokens[:4] if token not in {"requires", "true"}):
            return True, line.strip()
    return False, ""


def drop_requires_body_obligation(removed: str, mutant: str) -> tuple[bool, str]:
    """Body line that still relies on the removed precondition."""
    for line in mutant.splitlines():
        if line.strip().startswith(("requires", "ensures", "//", "machine ", "data ")):
            continue
        if any(
            token in line
            for token in re.findall(r"[A-Za-z_][\w]*", removed)
            if token not in {"requires", "requires", "&&", "||"}
        ):
            if re.search(r"/|\[|\.|<|>|==|!=", line):
                return True, line.strip()
    return False, ""


def classify_drop_requires(
    current: str,
    route: str,
    route_reason: str,
    original: str,
    mutant: str,
) -> ClassifyResult:
    removed = drop_requires_removed_line(original, mutant)
    if not removed:
        return ClassifyResult("discarded-valid", "no-requires-removed", None, "", route_reason)

    repeated, repeat_line = requires_fact_repeated_elsewhere(removed, mutant)
    body_need, body_line = drop_requires_body_obligation(removed, mutant)

    if current != "accepted":
        if route == "rejected" and route_reason:
            return ClassifyResult(
                "discarded-valid",
                "current-rejects",
                None,
                removed,
                route_reason,
            )
        return ClassifyResult("discarded-both-reject", "n/a", None, removed, route_reason)

    if route in ROUTE_NON_REJECT_REASONS:
        reason = f"route={route}"
        if route_reason:
            reason = f"{reason}: {route_reason}"
        return ClassifyResult(
            "discarded-valid",
            "route-non-proof-reject",
            None,
            removed,
            reason,
        )

    if route == "accepted":
        if body_need:
            return ClassifyResult(
                "confirmed-invalid",
                "confirmed-invalid",
                SPEC_RULES["drop_requires_line"],
                f"removed `{removed}`; body `{body_line}`",
                route_reason or "route=accepted",
            )
        return ClassifyResult(
            "discarded-valid",
            "accepted-without-body-obligation",
            None,
            removed,
            route_reason or "route=accepted",
        )

    if route == "rejected":
        if repeated and not body_need:
            note = f"requires duplicated in transition/ensures: removed `{removed}` vs `{repeat_line}`"
            return ClassifyResult(
                "discarded-valid",
                "requires-repeated",
                None,
                note,
                route_reason or "route=rejected",
            )
        if body_need:
            return ClassifyResult(
                "confirmed-invalid",
                "confirmed-invalid",
                SPEC_RULES["drop_requires_line"],
                f"removed `{removed}`; route rejected ({route_reason}); body `{body_line}`",
                route_reason,
            )
        return ClassifyResult(
            "discarded-valid",
            "route-rejects-no-body-obligation",
            None,
            removed,
            route_reason or "route=rejected",
        )

    return ClassifyResult(
        "discarded-valid",
        "unclassified-route",
        None,
        removed,
        route_reason,
    )


def obligation_for(kind: str, original: str, mutant: str) -> tuple[bool, str]:
    if kind == "remove_mut_binding":
        return remove_mut_obligation(original, mutant)
    if kind == "tighten_requires_equality":
        return tighten_requires_obligation(original, mutant)
    if kind == "demote_mut_ref_to_shared":
        return ref_demote_obligation(original, mutant)
    if kind == "promote_shared_ref_to_mut":
        return ref_promote_obligation(original, mutant)
    if kind == "corrupt_literal_callee_argument":
        return corrupt_call_obligation(original, mutant)
    if kind == "delete_transition_guard":
        if len(mutant) < len(original):
            line = _line_evidence(mutant, r"transition\s*\{")
            return (bool(line), line)
    return False, ""


def classify_mutant(
    kind: str,
    current: str,
    route: str,
    original: str,
    mutant: str,
    route_reason: str = "",
) -> ClassifyResult:
    if kind == "drop_requires_line":
        return classify_drop_requires(current, route, route_reason, original, mutant)

    timeout = current in ("timeout", "error", "unknown") or route in (
        "timeout",
        "unknown",
    )
    if timeout or "timeout" in current or "timeout" in route:
        return ClassifyResult(
            "discarded-timeout",
            "discarded-timeout",
            None,
            "",
            route_reason,
        )

    if current.startswith("exit_") or current == "error":
        return ClassifyResult(
            "discarded-run-failed",
            "omega-check-failed",
            None,
            "",
            route_reason,
        )

    has_obligation, obl_line = obligation_for(kind, original, mutant)
    rule = SPEC_RULES.get(kind)
    current_accepts = current == "accepted"
    route_accepts = route == "accepted"

    if current == "rejected" and route == "rejected":
        return ClassifyResult("discarded-both-reject", "n/a", None, "", route_reason)

    if current_accepts:
        if not has_obligation:
            return ClassifyResult(
                "discarded-valid", "discarded-valid", None, "", route_reason
            )
        if kind == "remove_mut_binding":
            return ClassifyResult(
                "known-gap-q7-mut",
                "known-gap-q7-mut",
                SPEC_RULES["remove_mut_binding"],
                obl_line,
                route_reason,
            )
        if not rule:
            return ClassifyResult(
                "discarded-no-rule",
                "discarded-no-rule",
                None,
                obl_line,
                route_reason,
            )
        return ClassifyResult(
            "confirmed-invalid",
            "confirmed-invalid",
            rule,
            obl_line,
            route_reason,
        )

    if route_accepts:
        if has_obligation:
            return ClassifyResult(
                "checker-disagreement",
                "route-accepted-current-rejected",
                rule,
                obl_line,
                route_reason,
            )
        return ClassifyResult(
            "discarded-valid", "discarded-valid", None, "", route_reason
        )

    if current != route:
        return ClassifyResult(
            "discarded-valid", "discarded-valid", None, "", route_reason
        )

    return ClassifyResult(
        "discarded-valid", "discarded-valid", None, "", route_reason
    )
