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
}

BUCKETS = (
    "confirmed-invalid",
    "checker-disagreement",
    "known-gap-q7-mut",
    "discarded-valid",
    "discarded-no-rule",
    "discarded-both-reject",
    "discarded-timeout",
)


@dataclass
class ClassifyResult:
    disposition: str
    hand_check: str
    spec: SpecRule | None
    obligation_line: str


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
            rf"\b{re.escape(bind)}\.[A-Za-z_0-9]+\s*=|&mut\s+{re.escape(bind)}\b",
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


def ref_demote_obligation(original: str, mutant: str) -> tuple[bool, str]:
    if "&mut " not in original or "&mut " in mutant:
        return False, ""
    write = _line_evidence(mutant, r"\w+\s*\.|\w+\s*\[.*\]\s*=|[^=]=[^=].*;")
    return (bool(write), write)


def ref_promote_obligation(original: str, mutant: str) -> tuple[bool, str]:
    if original.count("&mut") >= mutant.count("&mut"):
        return False, ""
    line = _line_evidence(mutant, r"&mut\s+\w+")
    return (bool(line), line)


def corrupt_call_obligation(_original: str, mutant: str) -> tuple[bool, str]:
    call = re.search(r"(?m)^\s*[A-Za-z_][\w:]*\([^)]*0[^)]*\)", mutant)
    if call:
        return True, call.group(0).strip()
    return False, ""


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
) -> ClassifyResult:
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
        )

    has_obligation, obl_line = obligation_for(kind, original, mutant)
    rule = SPEC_RULES.get(kind)
    current_accepts = current == "accepted"
    route_accepts = route == "accepted"

    if current == "rejected" and route == "rejected":
        return ClassifyResult("discarded-both-reject", "n/a", None, "")

    if current_accepts:
        if not has_obligation:
            return ClassifyResult("discarded-valid", "discarded-valid", None, "")
        if kind == "remove_mut_binding":
            return ClassifyResult(
                "known-gap-q7-mut",
                "known-gap-q7-mut",
                SPEC_RULES["remove_mut_binding"],
                obl_line,
            )
        if not rule:
            return ClassifyResult("discarded-no-rule", "discarded-no-rule", None, obl_line)
        return ClassifyResult(
            "confirmed-invalid",
            "confirmed-invalid",
            rule,
            obl_line,
        )

    # Current rejects: route-only acceptance is not a confirmed current hole.
    if route_accepts:
        if has_obligation:
            return ClassifyResult(
                "checker-disagreement",
                "route-accepted-current-rejected",
                rule,
                obl_line,
            )
        return ClassifyResult("discarded-valid", "discarded-valid", None, "")

    if current != route:
        return ClassifyResult("discarded-valid", "discarded-valid", None, "")

    return ClassifyResult("discarded-valid", "discarded-valid", None, "")
