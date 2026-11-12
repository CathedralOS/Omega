"""soundness_fix4: valid-route-outside, airtight confirmed holes."""

from __future__ import annotations

import re

from fix3_classify import Outcome, checker_side, accepted_by

ROUTE_OUTSIDE = frozenset({"outside", "no-entry", "unreached", "missing"})

CHECK_TIME_SPECS: dict[str, tuple[str, str, str]] = {
    "mut_ref_instead_of_shared": (
        "language/ownership.md",
        "## Owned local mutability",
        "modifying an established ordinary local's owned storage requires a `mut` binding.",
    ),
    "shared_instead_of_mut": (
        "language/ownership.md",
        "## Owned local mutability",
        "formation of `&mut` or `&write` loans to those places.",
    ),
    "drop_requires": (
        "language/machines.md",
        "## Nested machines and identity",
        "A written entry contract remains complete even when all currently visible callers happen to establish stronger facts.",
    ),
    "widen_requires": (
        "proofs/contracts.md",
        "## Machines and bundles",
        "Before importing a callee's conclusions, a caller establishes the assumptions under the exact argument substitution.",
    ),
}


def route_is_outside(route: str) -> bool:
    return route in ROUTE_OUTSIDE


def real_disagreement(omg: str, cur: str, route: str) -> bool:
    from fix3_classify import route_side

    cs, rs = checker_side(omg, cur), route_side(route)
    if cs == "accept" and rs == "reject":
        return True
    if cs == "reject" and rs == "accept":
        return True
    if (
        omg not in ("skipped", "")
        and omg in ("accepted", "rejected")
        and cur in ("accepted", "rejected")
        and omg != cur
    ):
        return True
    return False


def runtime_trap_deferred(kind: str, mutant: str, obligation: str | None) -> bool:
    if kind == "drop_guard":
        return True
    if kind in ("mut_ref_instead_of_shared", "shared_instead_of_mut"):
        return False
    if obligation and obligation.startswith("//"):
        return True
    if kind == "drop_requires" and re.search(r"transition.*bad\(\)|crash\s+Trap", mutant):
        if obligation and "[" not in obligation and "/" not in obligation:
            return True
    return False


def minimal_diff(original: str, mutant: str, max_lines: int = 6) -> str:
    lines: list[str] = []
    for a, b in zip(original.splitlines(), mutant.splitlines()):
        if a != b:
            lines.append(f"- {a}")
            lines.append(f"+ {b}")
        if len(lines) >= max_lines:
            break
    return "\n".join(lines)


def airtight_confirmed(
    kind: str,
    original: str,
    mutant: str,
    oc: Outcome,
) -> tuple[bool, str, str | None, str | None, str | None]:
    if not oc.obligation or oc.obligation.startswith("//"):
        return False, "confirmed downgrade: obligation is comment or empty", None, None, None

    spec = CHECK_TIME_SPECS.get(kind)
    if not spec and oc.spec_path and oc.spec_quote:
        spec = (oc.spec_path, oc.spec_heading or "", oc.spec_quote)
    if not spec:
        return False, "confirmed downgrade: no check-time spec mapping", None, None, None

    if runtime_trap_deferred(kind, mutant, oc.obligation):
        return False, "confirmed downgrade: runtime trap defers violation (fix4b)", None, None, None

    path, heading, quote = spec
    return True, "fix4 airtight", path, quote, heading


def apply_fix4(
    oc: Outcome,
    kind: str,
    original: str,
    mutant: str,
    omg: str,
    cur: str,
    route: str,
) -> Outcome:
    ab = accepted_by(omg, cur, route)
    cs = checker_side(omg, cur)

    if oc.valid and cs == "accept" and route_is_outside(route):
        if oc.bucket in ("discarded-valid", "disagreement-valid"):
            return Outcome(
                "valid-route-outside",
                "current",
                True,
                oc.obligation,
                None,
                None,
                None,
                f"valid mutant; route `{route}` not a checker verdict (fix4)",
            )

    if oc.bucket == "disagreement-valid" and not real_disagreement(omg, cur, route):
        if oc.valid and route_is_outside(route):
            return Outcome(
                "valid-route-outside",
                "current",
                True,
                oc.obligation,
                None,
                None,
                None,
                "reclassified from disagreement-valid (route outside)",
            )
        return Outcome(
            "discarded-valid",
            ab,
            oc.valid,
            oc.obligation,
            None,
            None,
            None,
            "reclassified: no accept-vs-reject disagree",
        )

    if oc.bucket == "confirmed-hole":
        ok, note, path, quote, heading = airtight_confirmed(kind, original, mutant, oc)
        if not ok:
            if cs == "accept" and route_is_outside(route):
                return Outcome(
                    "valid-route-outside",
                    "current",
                    True,
                    oc.obligation,
                    None,
                    None,
                    None,
                    note,
                )
            return Outcome(
                "discarded-valid",
                ab if cs == "accept" else "",
                True,
                oc.obligation,
                None,
                None,
                None,
                note,
            )
        merged = f"{oc.note} | {note}" if oc.note else note
        return Outcome(
            oc.bucket,
            ab,
            False,
            oc.obligation,
            path,
            quote,
            heading,
            merged,
        )

    return oc
