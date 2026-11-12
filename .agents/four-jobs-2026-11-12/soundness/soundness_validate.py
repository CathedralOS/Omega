"""Lead-review classification (fix3): validity from reading the mutant; holes if either checker accepts."""

from __future__ import annotations

import re
from dataclasses import dataclass

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

SPEC_MODULAR_ENTRY = (
    "wiki/spec/language/machines.md#nested-machines-and-identity",
    "A written entry contract remains complete even when all currently visible callers "
    "happen to establish stronger facts.",
)

SPEC_AUTHORED_REQUIRES = (
    "wiki/spec/language/state_contracts.md#private-entry-inference",
    "An authored `requires` is complete, including on a private helper. Inference "
    "cannot silently conjoin further entry conditions to make the body check.",
)

SPEC_NONZERO_RANGE = (
    "wiki/spec/language/expressions.md",
    "proof error, not a runtime panic. Thus `..=len-1` needs nonzero length, and an",
)

SPEC_REBORROW = (
    "wiki/spec/language/ownership.md",
    "Reborrowing retains exact immediate parent lineage. Shared children freeze the "
    "parent's mutation; exclusive children suspend the corresponding parent branch.",
)

ROUTE_NON_VERDICT = frozenset(
    {"outside", "no-entry", "unreached", "unsupported", "incomplete", "defect", "error"}
)


def route_is_accept(route: str) -> bool:
    return route == "accepted"


def route_is_reject(route: str) -> bool:
    return route == "rejected"


def route_is_non_verdict(route: str) -> bool:
    return route in ROUTE_NON_VERDICT


def runtime_trap_only(mutant: str, kind: str) -> bool:
    if "crashes Trap" not in mutant and "crash Trap" not in mutant:
        return False
    if _line_find(r"/", mutant) or _line_find(r"\[[^\]]+\]", mutant):
        return False
    return kind in {"drop_requires", "widen_requires", "drop_divisor_guard"}


def confirmed_hole_allowed(kind: str, mutant: str, spec_path: str) -> tuple[bool, str]:
    if runtime_trap_only(mutant, kind):
        return False, "violation only reaches a runtime Trap arm; not a check-time proof hole"
    if spec_path and "expressions.md" in spec_path:
        return True, "spec cites check-time proof error, not a runtime panic"
    if spec_path and "ownership.md" in spec_path:
        return True, "owned-local / borrow rules are checked obligations, not runtime traps"
    if spec_path and "state_contracts.md" in spec_path:
        return True, "machine requires/ensures are check-time contracts"
    return True, "check-time contract rule"


@dataclass
class Invalidity:
    invalid: bool
    obligation: str = ""
    spec_path: str = ""
    spec_quote: str = ""
    valid_reason: str = ""
    q7_only: bool = False
    hole_subtype: str = ""


def _line_find(pattern: str, text: str) -> str | None:
    for number, line in enumerate(text.splitlines(), 1):
        if re.search(pattern, line):
            return f"L{number}: {line.strip()}"
    return None


def demoted_mut_binding(original: str, mutant: str) -> str | None:
    om = list(re.finditer(r"\blet\s+mut\s+(\w+)", original))
    if not om:
        return None
    name = om[0].group(1)
    if re.search(rf"\blet\s+mut\s+{re.escape(name)}\b", mutant):
        return None
    if not re.search(rf"\blet\s+{re.escape(name)}\b", mutant):
        return None
    return name


def binding_shadowed(mutant: str, name: str) -> bool:
    return (
        sum(
            1
            for line in mutant.splitlines()
            if re.search(rf"\blet\s+(?:mut\s+)?{re.escape(name)}\b", line)
        )
        > 1
    )


def mutating_use(mutant: str, name: str) -> str | None:
    for pat in (
        rf"&mut\s+{re.escape(name)}\b",
        rf"&write\s+{re.escape(name)}\b",
        rf"\b{re.escape(name)}\s*\[[^\]]+\]\s*=",
        rf"\b{re.escape(name)}\.[A-Za-z_][\w]*\s*=",
        rf"(?<![\w.]){re.escape(name)}\s*=",
    ):
        hit = _line_find(pat, mutant)
        if hit:
            return hit
    return None


def extract_first_requires_clause(text: str) -> str | None:
    from requires_span import extract_first_requires_clause as _clause  # noqa: WPS433

    return _clause(text)


def internal_restatement(mutant: str, requires_clause: str) -> str | None:
    """Transition/guard inside the machine that restates the removed entry fact."""
    tokens: list[str] = []
    for frag in re.findall(r"!=\s*0\w*|==\s*0\w*|<\s*\w+|>\s*\w+|\w+\s*!=\s*\w+", requires_clause):
        tokens.append(frag)
    if not tokens and requires_clause:
        tokens = [requires_clause[: min(48, len(requires_clause))]]
    for number, line in enumerate(mutant.splitlines(), 1):
        if not re.search(r"\b(transition|match)\b", line):
            continue
        if any(tok in line for tok in tokens if tok):
            return f"L{number}: {line.strip()}"
    return None


def drop_requires_invalidity(original: str, mutant: str) -> Invalidity:
    """Fix5: invalid only if body still needs removed entry fact without internal restatement."""
    orig_req = extract_first_requires_clause(original)
    mut_req = extract_first_requires_clause(mutant)
    if not orig_req or mut_req:
        return Invalidity(False, valid_reason="requires not removed")

    modular_quote = (
        f"[modular] ({SPEC_MODULAR_ENTRY[0]}) {SPEC_MODULAR_ENTRY[1]} | "
        f"[authored] ({SPEC_AUTHORED_REQUIRES[0]}) {SPEC_AUTHORED_REQUIRES[1]}"
    )
    inside = internal_restatement(mutant, orig_req)
    if inside:
        return Invalidity(
            False,
            valid_reason=(
                f"internal transition/guard restates removed requires `{orig_req}` at {inside}; "
                f"outside callers cannot supply this fact ({SPEC_MODULAR_ENTRY[1]})"
            ),
        )

    for pat, spec in (
        (r"/", SPEC_NONZERO_RANGE),
        (r"\[[^\]]+\]", SPEC_INDEX_BOUNDS),
    ):
        hit = _line_find(pat, mutant)
        if hit:
            return Invalidity(
                True,
                hit,
                SPEC_MODULAR_ENTRY[0],
                modular_quote,
                hole_subtype="probe-not-built",
            )
    ensures = _line_find(r"\bensures\b", mutant)
    if ensures:
        return Invalidity(
            True,
            ensures,
            SPEC_AUTHORED_REQUIRES[0],
            modular_quote,
            hole_subtype="probe-not-built",
        )
    return Invalidity(
        False,
        valid_reason="removed requires not used by body without internal restatement",
    )


def remove_mut_invalidity(original: str, mutant: str) -> Invalidity:
    binding = demoted_mut_binding(original, mutant)
    if not binding:
        return Invalidity(False, valid_reason="no demoted let mut")
    if binding_shadowed(mutant, binding):
        return Invalidity(False, valid_reason=f"binding `{binding}` is shadowed")
    obligation = mutating_use(mutant, binding) or ""
    if not obligation:
        return Invalidity(False, valid_reason="demoted binding has no mutating use")
    q7 = bool(
        re.search(
            rf"&mut\s+{re.escape(binding)}\b|&write\s+{re.escape(binding)}\b", obligation
        )
    )
    return Invalidity(
        True,
        obligation,
        SPEC_OWNED_LOCAL_MUT[0],
        SPEC_OWNED_LOCAL_MUT[1],
        q7_only=q7,
    )


def weaken_requires_invalidity(original: str, mutant: str) -> Invalidity:
    if "!= 0" in original and "== 0" in mutant:
        div = _line_find(r"/", mutant)
        if div:
            return Invalidity(True, div, SPEC_NONZERO_RANGE[0], SPEC_NONZERO_RANGE[1])
    return Invalidity(False, valid_reason="weakened guard not used in body")


def flip_borrow_invalidity(original: str, mutant: str) -> Invalidity:
    for number, (o, m) in enumerate(zip(original.splitlines(), mutant.splitlines()), 1):
        if o == m:
            continue
        if "&mut" in m or "&write" in m or ("&" in m and "&mut" not in o and "&write" not in o):
            return Invalidity(
                True,
                f"L{number}: {m.strip()}",
                SPEC_REBORROW[0],
                SPEC_REBORROW[1],
            )
    hit = _line_find(r"&mut|&write", mutant)
    if hit and hit not in original:
        return Invalidity(True, hit, SPEC_REBORROW[0], SPEC_REBORROW[1])
    return Invalidity(False, valid_reason="borrow change not tied to a use line")


def assess_invalidity(kind: str, original: str, mutant: str) -> Invalidity:
    if kind == "drop_requires":
        return drop_requires_invalidity(original, mutant)
    if kind == "remove_mut":
        return remove_mut_invalidity(original, mutant)
    if kind in {"widen_requires", "drop_divisor_guard"}:
        return weaken_requires_invalidity(original, mutant)
    if kind == "flip_borrow_mutability":
        return flip_borrow_invalidity(original, mutant)
    if kind == "write_through_shared_borrow":
        hit = _line_find(r"\[[0-9]+\]\s*=", mutant)
        if hit:
            return Invalidity(True, hit, SPEC_REBORROW[0], SPEC_REBORROW[1])
    return Invalidity(False, valid_reason=f"no invalidity rule for kind {kind}")


def accepted_by(current: str, route: str) -> str:
    cur = current == "accepted"
    rte = route_is_accept(route)
    if cur and rte:
        return "both"
    if cur:
        return "current"
    if rte:
        return "route"
    return ""


def classify(
    kind: str,
    original: str,
    mutant: str,
    current: str,
    route: str,
    route_reason: str = "",
) -> tuple[str, str, str, str, str, str, str]:
    """hand_verdict, spec_path, spec_quote, obligation, review_reason, accepted_by, hole_subtype."""
    _ = route_reason  # recorded separately; never used to decide validity

    if kind == "_mutant_generate_failed":
        return "failed-run", "", "", "", "mutant directory staging failed", "", ""
    if kind.startswith("_"):
        return "no-mutation", "", "", "", "", "", ""

    if current == "timeout" or route == "timeout":
        return "timeout", "", "", "", "checker timeout", "", ""

    if current == "error" or route == "error":
        return "failed-run", "", "", "", "checker error", "", ""

    cur_ok = current == "accepted"
    rte_ok = route_is_accept(route)
    acc = accepted_by(current, route)
    inv = assess_invalidity(kind, original, mutant)

    if inv.invalid:
        if inv.q7_only and (cur_ok or rte_ok):
            return (
                "known-gap-q7-mut",
                inv.spec_path,
                inv.spec_quote,
                inv.obligation,
                f"Q7 missing-mut enforcement on `{inv.obligation}`",
                acc,
                inv.hole_subtype,
            )
        if cur_ok or rte_ok:
            ok, why = confirmed_hole_allowed(kind, mutant, inv.spec_path)
            if not ok:
                return "discarded-valid", "", "", inv.obligation, why, "", ""
            return (
                "confirmed-hole",
                inv.spec_path,
                inv.spec_quote,
                inv.obligation,
                f"modular-entry hole: {why}",
                acc,
                inv.hole_subtype or ("probe-not-built" if kind == "drop_requires" else ""),
            )
        return "rejected-both", "", "", inv.obligation, "invalid mutant rejected by both checkers", "", ""

    if cur_ok and rte_ok:
        return "discarded-valid", "", "", "", inv.valid_reason or "valid mutant; both checkers accept", "", ""
    if cur_ok and route_is_reject(route):
        return (
            "disagreement-valid",
            "",
            "",
            "",
            inv.valid_reason or "valid mutant; current accepts, route rejects",
            "current",
            "",
        )
    if rte_ok and current == "rejected":
        return (
            "disagreement-valid",
            "",
            "",
            "",
            inv.valid_reason or "valid mutant; route accepts, current rejects",
            "route",
            "",
        )
    if cur_ok and route_is_non_verdict(route):
        return (
            "valid-route-outside",
            "",
            "",
            "",
            f"valid mutant; route non-verdict `{route}`",
            "",
            "",
        )
    return "rejected-both", "", "", "", inv.valid_reason or "valid mutant; both reject", "", ""
