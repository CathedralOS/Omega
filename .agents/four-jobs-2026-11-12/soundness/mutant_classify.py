"""Soundness mutation disposition (lead review fix3 wins over prior notes)."""

from __future__ import annotations

import re
from dataclasses import dataclass


@dataclass(frozen=True)
class SpecRule:
    heading: str
    path: str
    quote: str


SPEC_RULES: dict[str, SpecRule] = {
    "drop_requires_line": SpecRule(
        "Nested machines and identity; Private entry inference",
        "wiki/spec/language/machines.md; wiki/spec/language/state_contracts.md",
        "A written entry contract remains complete even when all currently visible callers "
        "happen to establish stronger facts. An authored `requires` is complete, including "
        "on a private helper. Inference cannot silently conjoin further entry conditions "
        "to make the body check.",
    ),
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
    "mut_ref_instead_of_shared": SpecRule(
        "Borrows and aliases",
        "wiki/spec/language/ownership.md",
        "`&mut T` | Exclusive observation and mutation.",
    ),
    "shared_instead_of_mut": SpecRule(
        "Borrows and aliases",
        "wiki/spec/language/ownership.md",
        "`&T` | Shared observation under its loan contract.",
    ),
    "corrupt_literal_callee_argument": SpecRule(
        "Machine requirements at invocation",
        "wiki/spec/language/state_contracts.md",
        "Machine `requires` applies at every invocation, including terminal and recursive "
        "calls; every arrival establishes the target's requirements under actual argument substitution.",
    ),
    "delete_transition_guard": SpecRule(
        "Transitions",
        "wiki/spec/language/state_contracts.md",
        "Transition guards establish that only enabled edges may fire.",
    ),
}

ROUTE_OUTSIDE = frozenset(
    {"no-entry", "unreached", "outside", "fallback", "incomplete", "defect", "stopped"}
)

BUCKETS = (
    "confirmed-hole",
    "known-gap-q7-mut",
    "disagreement-valid",
    "valid-route-outside",
    "discarded-valid",
    "rejected-both",
    "timeout",
    "failed-run",
    "discarded-no-rule",
)


def map_kind(kind: str) -> str:
    if kind == "demote_mut_ref_to_shared":
        return "mut_ref_instead_of_shared"
    if kind == "promote_shared_ref_to_mut":
        return "shared_instead_of_mut"
    return kind


@dataclass
class Validity:
    invalid: bool
    obligation_line: str
    validity_note: str
    spec: SpecRule | None
    q7_only: bool


@dataclass
class ClassifyResult:
    disposition: str
    hand_check: str
    spec: SpecRule | None
    obligation_line: str
    route_reason: str
    accepted_by: str
    validity_note: str
    hole_subtype: str = ""


def _line_evidence(text: str, pattern: str) -> str:
    for line in text.splitlines():
        if re.search(pattern, line):
            return line.strip()
    return ""


def drop_requires_removed_line(original: str, mutant: str) -> str:
    orig_reqs = [line.strip() for line in original.splitlines() if line.strip().startswith("requires")]
    mut_set = {line.strip() for line in mutant.splitlines()}
    for req in orig_reqs:
        if req not in mut_set:
            return req
    return ""


def _requires_content_tokens(removed: str) -> list[str]:
    return [
        token
        for token in re.findall(r"[A-Za-z_][\w]*|\d+", removed)
        if token not in {"requires", "true", "&&", "||"}
    ]


def internal_restates_removed(removed: str, mutant: str) -> tuple[bool, str]:
    """Transition/guard inside the machine restates the removed requires (mutant stays valid)."""
    tokens = _requires_content_tokens(removed)
    if not tokens:
        return False, ""
    for line in mutant.splitlines():
        stripped = line.strip()
        inside = stripped.startswith("transition") or re.match(r"^if\s+", stripped)
        if not inside and "->" not in stripped:
            continue
        hit = sum(1 for token in tokens[:6] if token in stripped)
        if hit >= 2 or (hit >= 1 and re.search(r"==|<=|>=|<|>", stripped)):
            return True, stripped
    return False, ""


def drop_requires_validity(original: str, mutant: str) -> Validity:
    removed = drop_requires_removed_line(original, mutant)
    if not removed:
        return Validity(False, "", "no requires removed", None, False)
    restated, restate_line = internal_restates_removed(removed, mutant)
    if restated:
        return Validity(
            False,
            f"removed `{removed}` | restated `{restate_line}`",
            "internal transition/guard restates removed requires (valid; outside-caller modular rule N/A)",
            None,
            False,
        )
    tokens = _requires_content_tokens(removed)
    for line in mutant.splitlines():
        stripped = line.strip()
        if stripped.startswith(("requires", "machine ", "data ", "//")):
            continue
        if stripped.startswith("ensures"):
            if any(token in stripped for token in tokens[:4]):
                return Validity(
                    True,
                    stripped,
                    "ensures still references facts from removed requires",
                    SPEC_RULES["drop_requires_line"],
                    False,
                )
        if tokens and any(token in line for token in tokens[:4]):
            if re.search(r"/|\[|\.|<|>|==|!=", line):
                return Validity(
                    True,
                    stripped,
                    f"body uses facts from removed `{removed}`",
                    SPEC_RULES["drop_requires_line"],
                    False,
                )
    return Validity(
        False,
        removed,
        "nothing in body/ensures needs the removed requires",
        None,
        False,
    )


def remove_mut_obligation(original: str, mutant: str) -> tuple[bool, str]:
    bind = None
    for match in re.finditer(r"(?m)^(\s*)let mut ([A-Za-z_][\w]*)", original):
        name = match.group(2)
        demoted = f"{match.group(1)}let {name}"
        if demoted in mutant and match.group(0) not in mutant:
            bind = name
            break
    if not bind:
        return False, ""
    past_binding = False
    for line in mutant.splitlines():
        if re.match(rf"^\s*let\s+{re.escape(bind)}\b", line) and "let mut" not in line:
            past_binding = True
            continue
        if not past_binding:
            continue
        if re.match(rf"^\s*let\s+{re.escape(bind)}\b", line) or re.match(
            rf"^\s*let\s+mut\s+{re.escape(bind)}\b", line
        ):
            return False, ""
        if re.search(
            rf"\b{re.escape(bind)}\s*=|\b{re.escape(bind)}\[[^\]]*\]\s*=|"
            rf"\b{re.escape(bind)}\.[A-Za-z_0-9]+\s*=|"
            rf"&mut\s+{re.escape(bind)}\b|&write\s+{re.escape(bind)}\b",
            line,
        ):
            return True, line.strip()
    return False, ""


def _param_ref_swaps(original: str, mutant: str) -> list[tuple[str, str, str]]:
    """Return (name, from_mode, to_mode) for parameter reference-mode edits."""
    swaps: list[tuple[str, str, str]] = []
    for line_o, line_m in zip(original.splitlines(), mutant.splitlines()):
        if line_o == line_m:
            continue
        for match in re.finditer(r"([A-Za-z_][\w]*)\s*:\s*&mut\b", line_o):
            name = match.group(1)
            if re.search(rf"{re.escape(name)}\s*:\s*&\b", line_m):
                swaps.append((name, "mut", "shared"))
        for match in re.finditer(r"([A-Za-z_][\w]*)\s*:\s*&\b", line_o):
            name = match.group(1)
            if re.search(rf"{re.escape(name)}\s*:\s*&mut\b", line_m):
                swaps.append((name, "shared", "mut"))
    return swaps


def ref_mutation_validity(kind: str, original: str, mutant: str) -> Validity:
    rule_key = map_kind(kind)
    rule = SPEC_RULES.get(rule_key) or SPEC_RULES.get(kind)
    for name, _from, to_mode in _param_ref_swaps(original, mutant):
        for line in mutant.splitlines():
            if re.search(
                rf"\b{name}\s*\.[A-Za-z_0-9]+\s*=|\b{name}\s*\[[^\]]*\]\s*=|\b{name}\s*=",
                line,
            ):
                if to_mode == "shared":
                    return Validity(
                        True,
                        line.strip(),
                        f"write through parameter `{name}` after demoting &mut to &",
                        rule,
                        False,
                    )
                return Validity(
                    True,
                    line.strip(),
                    f"write through `{name}` after promoting & to &mut",
                    rule,
                    False,
                )
    return Validity(False, "", "no write through affected reference parameter", rule, False)


def tighten_requires_validity(original: str, mutant: str) -> Validity:
    if original == mutant:
        return Validity(False, "", "requires unchanged", None, False)
    call = _line_evidence(mutant, r"^\s*[A-Za-z_:][\w:]*\([^)]*\)")
    req = _line_evidence(mutant, r"^\s*requires\b")
    if call and req:
        return Validity(
            True,
            f"{req} with call `{call}`",
            "call cannot establish tightened requires",
            SPEC_RULES["tighten_requires_equality"],
            False,
        )
    return Validity(False, "", "no call/requires tension visible", None, False)


def corrupt_call_validity(_original: str, mutant: str) -> Validity:
    call = _line_evidence(mutant, r"^\s*[A-Za-z_:][\w:]*\([^)]*0[^)]*\)")
    if call:
        return Validity(
            True,
            call,
            "literal argument disagrees with callee requires",
            SPEC_RULES["corrupt_literal_callee_argument"],
            False,
        )
    return Validity(False, "", "no corrupted call literal", None, False)


def assess_validity(kind: str, original: str, mutant: str) -> Validity:
    kind = map_kind(kind)
    if kind == "drop_requires_line":
        return drop_requires_validity(original, mutant)
    if kind == "remove_mut_binding":
        ok, line = remove_mut_obligation(original, mutant)
        if not ok:
            return Validity(False, "", "no write on demoted binding", None, False)
        return Validity(
            True,
            line,
            "write or &mut/&write on non-mut local",
            SPEC_RULES["remove_mut_binding"],
            True,
        )
    if kind in ("demote_mut_ref_to_shared", "promote_shared_ref_to_mut", "mut_ref_instead_of_shared", "shared_instead_of_mut"):
        return ref_mutation_validity(kind, original, mutant)
    if kind == "tighten_requires_equality":
        return tighten_requires_validity(original, mutant)
    if kind == "corrupt_literal_callee_argument":
        return corrupt_call_validity(original, mutant)
    if kind == "delete_transition_guard" and len(mutant) < len(original):
        line = _line_evidence(mutant, r"transition\s*\{")
        if line:
            return Validity(
                True,
                line,
                "guard removed from transition",
                SPEC_RULES["delete_transition_guard"],
                False,
            )
    return Validity(False, "", f"no invalidity rule for kind `{kind}`", None, False)


def route_accepts(verdict: str) -> bool:
    return verdict == "accepted"


def route_rejects(verdict: str) -> bool:
    return verdict == "rejected"


def route_outside(verdict: str) -> bool:
    return verdict in ROUTE_OUTSIDE


def current_accepts(verdict: str) -> bool:
    return verdict == "accepted"


def likely_runtime_trapped(mutant: str) -> bool:
    """Heuristic: violation may only be caught on a crash/transition path."""
    return bool(
        re.search(
            r"\bbad\(\)|crashes\b|machine\s+\w+\s*\(\)\s*\{\s*\}\s*$|->\s*bad\(\)",
            mutant,
            re.MULTILINE,
        )
    )


def compile_time_obligation(kind: str, validity: Validity) -> bool:
    """Owner fix4: hole must be about check-time proof, not runtime trap."""
    if kind in ("remove_mut_binding", "demote_mut_ref_to_shared", "promote_shared_ref_to_mut",
                "mut_ref_instead_of_shared", "shared_instead_of_mut"):
        return True
    if kind in ("drop_requires_line", "tighten_requires_equality", "corrupt_literal_callee_argument"):
        return True
    return bool(validity.spec)


def minimal_mutant_diff(original: str, mutant: str) -> str:
    orig_lines = original.splitlines()
    mut_lines = mutant.splitlines()
    chunks: list[str] = []
    for index, (left, right) in enumerate(zip(orig_lines, mut_lines)):
        if left != right:
            chunks.append(f"- {left}")
            chunks.append(f"+ {right}")
    if len(mut_lines) > len(orig_lines):
        for extra in mut_lines[len(orig_lines) :]:
            chunks.append(f"+ {extra}")
    elif len(orig_lines) > len(mut_lines):
        for gone in orig_lines[len(mut_lines) :]:
            chunks.append(f"- {gone}")
    return "\n".join(chunks[:40])


def classify_mutant(
    kind: str,
    current: str,
    route: str,
    original: str,
    mutant: str,
    route_reason: str = "",
) -> ClassifyResult:
    if current in ("timeout", "unknown") or route in ("timeout", "unknown"):
        return ClassifyResult(
            "timeout",
            "timeout",
            None,
            "",
            route_reason,
            "",
            "",
        )
    if current.startswith("exit_") or current == "error":
        return ClassifyResult(
            "failed-run",
            "omega-check-failed",
            None,
            "",
            route_reason,
            "",
            "",
        )

    validity = assess_validity(kind, original, mutant)
    cur_ok = current_accepts(current)
    route_ok = route_accepts(route)

    if not validity.invalid:
        note = validity.validity_note
        if cur_ok and route_ok:
            return ClassifyResult(
                "discarded-valid",
                "discarded-valid",
                None,
                validity.obligation_line,
                route_reason,
                "",
                note,
            )
        if route_outside(route):
            return ClassifyResult(
                "valid-route-outside",
                "valid-route-outside",
                None,
                validity.obligation_line,
                route_reason,
                "",
                note,
            )
        if (cur_ok and route_rejects(route)) or (route_ok and not cur_ok):
            return ClassifyResult(
                "disagreement-valid",
                "disagreement-valid",
                None,
                validity.obligation_line,
                route_reason,
                "",
                note,
            )
        return ClassifyResult(
            "rejected-both",
            "n/a",
            None,
            validity.obligation_line,
            route_reason,
            "",
            note,
        )

    # Invalid under spec (from mutant reading).
    if validity.q7_only and (cur_ok or route_ok):
        accepted_by = "both" if cur_ok and route_ok else ("current" if cur_ok else "route")
        return ClassifyResult(
            "known-gap-q7-mut",
            "known-gap-q7-mut",
            validity.spec,
            validity.obligation_line,
            route_reason,
            accepted_by,
            validity.validity_note,
        )

    accepted_by = ""
    if cur_ok and route_ok:
        accepted_by = "both"
    elif cur_ok:
        accepted_by = "current"
    elif route_ok:
        accepted_by = "route"

    if accepted_by and validity.spec and validity.obligation_line:
        if not compile_time_obligation(kind, validity):
            return ClassifyResult(
                "discarded-valid",
                "runtime-trap-or-non-proof-spec",
                validity.spec,
                validity.obligation_line,
                route_reason,
                accepted_by,
                "spec obligation may be runtime-only",
            )
        if likely_runtime_trapped(mutant) and kind not in (
            "remove_mut_binding",
            "demote_mut_ref_to_shared",
            "promote_shared_ref_to_mut",
        ):
            return ClassifyResult(
                "discarded-valid",
                "runtime-trap-present",
                validity.spec,
                validity.obligation_line,
                route_reason,
                accepted_by,
                "mutant retains explicit crash/bad route",
            )
        return ClassifyResult(
            "confirmed-hole",
            "confirmed-hole",
            validity.spec,
            validity.obligation_line,
            route_reason,
            accepted_by,
            validity.validity_note,
        )

    if accepted_by:
        return ClassifyResult(
            "discarded-no-rule",
            "accepted-without-quoted-obligation",
            None,
            validity.obligation_line,
            route_reason,
            accepted_by,
            validity.validity_note,
        )

    return ClassifyResult(
        "rejected-both",
        "n/a",
        validity.spec,
        validity.obligation_line,
        route_reason,
        "",
        validity.validity_note,
    )
