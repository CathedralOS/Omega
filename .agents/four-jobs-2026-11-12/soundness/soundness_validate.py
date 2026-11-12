"""Lead-review classification for soundness mutation rows."""

from __future__ import annotations

import re

SPEC_OWNED_LOCAL_MUT = (
    "wiki/spec/language/ownership.md",
    "Under `immutable-binding-exclusive-loan` (owner ruling, 2026-11-12), modifying an established "
    "ordinary local's owned storage requires a `mut` binding. This covers direct "
    "assignment, writes to owned fields or array elements, and formation of "
    "`&mut` or `&write` loans to those places.",
)

SPEC_INDEX_BOUNDS = (
    "wiki/spec/language/expressions.md",
    "machine [] read<Element [copy]>(items: &[Element], index: u64) -> Element\n"
    "requires index < items.len;",
)

SPEC_REQUIRES_ARRIVAL = (
    "wiki/spec/language/state_contracts.md",
    "Machine `requires` applies at every invocation, including terminal and recursive "
    "calls. Every arrival establishes the target's requirements and declared domains "
    "under actual argument substitution.",
)

SPEC_NONZERO_RANGE = (
    "wiki/spec/language/expressions.md",
    "proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an",
)

ROUTE_AUTO_DISCARD = {
    "no-entry",
    "unreached",
    "outside",
    "unsupported",
    "incomplete",
    "defect",
    "error",
    "timeout",
}


def _line_find(pattern: str, text: str) -> str | None:
    for number, line in enumerate(text.splitlines(), 1):
        if re.search(pattern, line):
            return f"L{number}: {line.strip()}"
    return None


def demoted_mut_binding(original: str, mutant: str) -> str | None:
    om = list(re.finditer(r"\blet\s+mut\s+(\w+)", original))
    if not om:
        return None
    first = om[0].group(1)
    if re.search(rf"\blet\s+mut\s+{re.escape(first)}\b", mutant):
        return None
    if not re.search(rf"\blet\s+{re.escape(first)}\b", mutant):
        return None
    return first


def binding_shadowed(mutant: str, name: str) -> bool:
    hits = 0
    for line in mutant.splitlines():
        if re.search(rf"\blet\s+(?:mut\s+)?{re.escape(name)}\b", line):
            hits += 1
    return hits > 1


def mutating_use(mutant: str, name: str) -> str | None:
    patterns = [
        rf"&mut\s+{re.escape(name)}\b",
        rf"&write\s+{re.escape(name)}\b",
        rf"\b{re.escape(name)}\s*\[[^\]]+\]\s*=",
        rf"\b{re.escape(name)}\.[A-Za-z_][\w]*\s*=",
    ]
    for pat in patterns:
        hit = _line_find(pat, mutant)
        if hit:
            return hit
    assign = _line_find(rf"(?<![\w.]){re.escape(name)}\s*=", mutant)
    if assign and "&" not in assign.split("=", 1)[0]:
        return assign
    return None


def weakened_requires_obligation(original: str, mutant: str) -> str | None:
    if "!= 0" in original and "== 0" in mutant:
        div = _line_find(r"/", mutant)
        if div:
            return div
    removed = "requires" in original and not re.search(r"^\s*requires\s", mutant, re.M)
    if removed:
        for pat in (r"\[[^\]]+\]", r"/", r"ensures\s", r"transition\s"):
            hit = _line_find(pat, mutant)
            if hit:
                return hit
    return None


def flip_borrow_obligation(original: str, mutant: str) -> str | None:
    for number, (oline, mline) in enumerate(
        zip(original.splitlines(), mutant.splitlines()), 1
    ):
        if oline != mline and ("&mut" in mline or "&write" in mline):
            return f"L{number}: {mline.strip()}"
    return _line_find(r"&mut|&write", mutant)


def same_fact_in_transition(original: str, removed_requires: str) -> str | None:
    """If transition restates removed requires, return the transition line (else None)."""
    req_line = removed_requires.strip()
    for line in original.splitlines():
        if "transition" in line and req_line and req_line in line:
            return line.strip()
    return None


def classify(
    kind: str,
    original: str,
    mutant: str,
    current: str,
    route: str,
    route_reason: str = "",
) -> tuple[str, str, str, str, str]:
    """Return hand_verdict, spec_path, spec_quote, obligation_line, review_reason."""
    if kind.startswith("_"):
        return "discarded-valid", "", "", "", ""

    if kind == "drop_requires":
        if current != "accepted":
            return (
                "discarded-valid",
                "",
                "",
                "",
                f"current={current} (not accepted; no soundness signal)",
            )
        reason = route_reason or f"route={route}"
        if route in ROUTE_AUTO_DISCARD or "unsupported" in reason.lower():
            return (
                "discarded-valid",
                "",
                "",
                "",
                f"route non-proof refusal: {route}; {reason[:240]}",
            )
        obligation = weakened_requires_obligation(original, mutant) or ""
        proof_reject = (
            route == "rejected"
            and obligation
            and any(
                token in reason.lower()
                for token in ("cannot prove", "unproved", "rejected", "diagnostic")
            )
        )
        if proof_reject:
            spec_path, spec_quote = SPEC_REQUIRES_ARRIVAL
            dup = same_fact_in_transition(original, obligation)
            if dup:
                return (
                    "discarded-valid",
                    "",
                    "",
                    obligation,
                    f"requires duplicated in transition: `{dup}`",
                )
            return (
                "confirmed-invalid",
                spec_path,
                spec_quote,
                obligation,
                f"route rejected obligation: {reason[:240]}",
            )
        if route == "accepted" and obligation:
            return (
                "confirmed-invalid",
                SPEC_REQUIRES_ARRIVAL[0],
                SPEC_REQUIRES_ARRIVAL[1],
                obligation,
                "both checkers accepted after requires removal",
            )
        return (
            "discarded-valid",
            "",
            "",
            obligation,
            f"insufficient route evidence: {route}; {reason[:240]}",
        )

    if current != "accepted":
        return "discarded-valid", "", "", "", f"current={current}"

    if kind == "remove_mut":
        binding = demoted_mut_binding(original, mutant)
        if not binding or binding_shadowed(mutant, binding):
            return (
                "discarded-valid",
                "",
                "",
                "",
                "missing demoted binding or shadowed name",
            )
        obligation = mutating_use(mutant, binding) or ""
        if not obligation:
            return "discarded-valid", "", "", "", "no mutating use of demoted binding"
        spec_path, spec_quote = SPEC_OWNED_LOCAL_MUT
        if re.search(
            rf"&mut\s+{re.escape(binding)}\b|&write\s+{re.escape(binding)}\b", obligation
        ):
            return (
                "known-gap-q7-mut",
                spec_path,
                spec_quote,
                obligation,
                f"Q7 gap: writable loan/store on non-mut `{binding}`",
            )
        return (
            "confirmed-invalid",
            spec_path,
            spec_quote,
            obligation,
            "current accepts illegal write on non-mut local",
        )

    if kind in {"widen_requires", "drop_divisor_guard"}:
        obligation = weakened_requires_obligation(original, mutant) or ""
        if not obligation:
            return "discarded-valid", "", "", "", "no dependent body obligation"
        if "/" in obligation:
            spec_path, spec_quote = SPEC_NONZERO_RANGE
        elif "[" in obligation:
            spec_path, spec_quote = SPEC_INDEX_BOUNDS
        else:
            spec_path, spec_quote = SPEC_REQUIRES_ARRIVAL
        return (
            "confirmed-invalid",
            spec_path,
            spec_quote,
            obligation,
            f"weakened guard with body use; route={route}",
        )

    if kind == "flip_borrow_mutability":
        obligation = flip_borrow_obligation(original, mutant) or ""
        if not obligation:
            return "discarded-valid", "", "", "", "borrow flip did not change a use line"
        spec_path = "wiki/spec/language/ownership.md"
        spec_quote = (
            "Reborrowing retains exact immediate parent lineage. Shared children freeze the "
            "parent's mutation; exclusive children suspend the corresponding parent branch."
        )
        return (
            "confirmed-invalid",
            spec_path,
            spec_quote,
            obligation,
            f"current accepted borrow mutability change; route={route}",
        )

    return "discarded-no-rule", "", "", "", f"unclassified kind {kind}"
