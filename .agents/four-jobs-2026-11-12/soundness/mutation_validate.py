"""Lead-review validation for soundness mutation shards (soundness_fix rules)."""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path

REQUIRES_RE = re.compile(r"^\s*requires\s+(.+)$", re.MULTILINE)
ENSURES_RE = re.compile(r"^\s*ensures\s+(.+)$", re.MULTILINE)
LET_BIND_RE = re.compile(r"\blet\s+(mut\s+)?([A-Za-z_][A-Za-z0-9_]*)")


@dataclass
class Validation:
    bucket: str
    spec_path: str | None
    spec_quote: str | None
    spec_heading: str | None
    obligation_line: str | None
    notes: str


SPEC_BY_KIND: dict[str, tuple[str, str, str]] = {
    # kind -> (path, heading, quote)
    "remove_mut": (
        "language/ownership.md",
        "## Owned local mutability",
        "modifying an established ordinary local's owned storage requires a `mut` binding.",
    ),
    "drop_mut": (
        "language/ownership.md",
        "## Owned local mutability",
        "modifying an established ordinary local's owned storage requires a `mut` binding.",
    ),
    "remove_mut": (
        "language/ownership.md",
        "## Owned local mutability",
        "modifying an established ordinary local's owned storage requires a `mut` binding.",
    ),
    "shared_instead_of_mut": (
        "language/ownership.md",
        "## Owned local mutability",
        "formation of `&mut` or `&write` loans to those places.",
    ),
    "mut_ref_instead_of_shared": (
        "language/ownership.md",
        "## Owned local mutability",
        "A callee cannot grant writable access that the caller's place lacks.",
    ),
    "widen_requires": (
        "proofs/contracts.md",
        "## Machines and bundles",
        "Before importing a callee's conclusions, a caller establishes the assumptions under the exact argument substitution.",
    ),
    "break_ensures": (
        "proofs/contracts.md",
        "## Machines and bundles",
        "A written `ensures` remains an obligation and a minimum guarantee",
    ),
    "drop_guard": (
        "language/state_contracts.md",
        "## Requirement clauses",
        "Repeated predicate clauses in one scope conjoin their obligations.",
    ),
}


def current_accepts(omega: str, cur: str) -> bool:
    if omega == "accepted":
        return True
    if omega == "skipped" and cur == "accepted":
        return True
    return False


def removed_requires_clauses(original: str, mutant: str) -> list[str]:
    out: list[str] = []
    for m in REQUIRES_RE.finditer(original):
        clause = m.group(1).strip()
        if clause not in mutant:
            out.append(clause)
    return out


def transition_restates_fact(mutant: str, clause: str) -> tuple[bool, str, str]:
    """Return whether a transition line restates the removed requires (fix2: quote both)."""
    norm = re.sub(r"\s+", " ", clause.strip())
    for ln in mutant.splitlines():
        s = ln.strip()
        if not s.startswith("transition"):
            continue
        tnorm = re.sub(r"\s+", " ", s)
        if norm in tnorm or tnorm in norm:
            return True, clause, s
        # Same conjuncts in different order (weak check).
        parts = [p.strip() for p in re.split(r"&&|\|\|", norm) if p.strip()]
        if parts and all(p in tnorm for p in parts):
            return True, clause, s
    return False, clause, ""


def route_uninformative(route: str, route_reason: str) -> bool:
    if route in ("outside", "no-entry", "unreached", "missing", "timeout", "error"):
        return True
    low = route_reason.lower()
    return low.startswith("unsupported") or "unsupported:" in low


def classify_drop_requires(
    original: str,
    mutant: str,
    omega: str,
    cur: str,
    route: str,
    route_reason: str = "",
) -> Validation:
    removed = removed_requires_clauses(original, mutant)
    if not removed:
        return Validation("discarded-valid", None, None, None, None, "no removed requires")

    if not current_accepts(omega, cur):
        return Validation("rejected-both", None, None, None, None, "")

    if route_uninformative(route, route_reason):
        return Validation(
            "discarded-valid",
            None,
            None,
            None,
            None,
            f"route uninformative ({route}): {route_reason or route}",
        )

    clause = removed[0]
    restated, req_q, trans_q = transition_restates_fact(mutant, clause)
    if restated:
        return Validation(
            "discarded-valid",
            None,
            None,
            None,
            trans_q,
            f"requires duplicated in transition | requires: {req_q} | transition: {trans_q}",
        )

    if route == "rejected" and route_reason:
        low = route_reason.lower()
        if "unproved" in low or "obligation" in low or "diagnostic" in low:
            obl = first_non_comment_code_line(mutant, ("/", "[", "transition", "call"))
            return Validation(
                "confirmed-hole",
                "proofs/contracts.md",
                "Before importing a callee's conclusions, a caller establishes the assumptions under the exact argument substitution.",
                "## Machines and bundles",
                obl or route_reason[:200],
                f"route proof failure: {route_reason[:300]}",
            )

    # Body still uses facts from removed requires without restating them.
    for ln in mutant.splitlines():
        s = ln.strip()
        if s.startswith("//") or not s:
            continue
        if "[" in s or re.search(r"\b\w+\s*\(", s):
            if any(tok in s for tok in re.findall(r"\w+", clause)[:4]):
                if clause not in s:
                    return Validation(
                        "confirmed-hole",
                        "proofs/contracts.md",
                        "A written `requires` is complete, including on private helpers, under entry inference.",
                        "## Machines and bundles",
                        s,
                        f"dropped requires: {clause}",
                    )

    return Validation(
        "discarded-valid",
        None,
        None,
        None,
        None,
        "current accepts; no proven missing entry obligation",
    )


def first_dropped_mut_binding(original: str, mutant: str) -> str | None:
    orig_pairs = [(m.group(1) is not None, m.group(2)) for m in LET_BIND_RE.finditer(original)]
    mut_pairs = [(m.group(1) is not None, m.group(2)) for m in LET_BIND_RE.finditer(mutant)]
    for (om, on), (mm, mn) in zip(orig_pairs, mut_pairs):
        if om and not mm and on == mn:
            return on
    return None


def binding_write_obligation(name: str, src: str) -> str | None:
    """Illegal write/&mut/&write on the same binding (fix2: no shadowing)."""
    lines = src.splitlines()
    mut_at_decl: bool | None = None
    shadow_depth = 0
    for ln in lines:
        m = LET_BIND_RE.search(ln)
        if m:
            bname = m.group(2)
            is_mut = m.group(1) is not None
            if bname == name:
                if mut_at_decl is None:
                    mut_at_decl = is_mut
                else:
                    shadow_depth += 1
            continue
        if shadow_depth:
            continue
        if mut_at_decl is not False:
            continue
        stripped = ln.strip()
        if stripped.startswith("//"):
            continue
        if re.search(rf"\b{re.escape(name)}\s*=", ln):
            return stripped
        if re.search(rf"&mut\s+{re.escape(name)}\b", ln) or re.search(
            rf"&write\s+{re.escape(name)}\b", ln
        ):
            return stripped
        if re.search(rf"{re.escape(name)}\.", ln) and "=" in ln:
            return stripped
    return None


def classify_remove_mut(
    original: str, mutant: str, omega: str, cur: str, route: str
) -> Validation:
    name = first_dropped_mut_binding(original, mutant)
    if not name:
        return Validation("discarded-valid", None, None, None, None, "no mut removal detected")
    obl = binding_write_obligation(name, mutant)
    if not obl:
        return Validation("discarded-valid", None, None, None, None, "binding not written or reborrowed")
    if not current_accepts(omega, cur):
        return Validation("rejected-both", None, None, None, obl, "current checker rejects")
    path, heading, quote = SPEC_BY_KIND["drop_mut"]
    return Validation(
        "known-gap-q7-mut",
        path,
        quote,
        heading,
        obl,
        f"binding `{name}` lacks `mut` but is written or `&mut`-loaned",
    )


def classify_break_ensures(mutant: str, omega: str, cur: str, route: str) -> Validation:
    m = ENSURES_RE.search(mutant)
    if not m or "!" in m.group(1):
        pass
    if "!" in (m.group(1) if m else ""):
        if current_accepts(omega, cur):
            path, heading, quote = SPEC_BY_KIND["break_ensures"]
            for ln in mutant.splitlines():
                if "transition" in ln or "->" in ln or "return" in ln:
                    return Validation(
                        "confirmed-hole",
                        path,
                        quote,
                        heading,
                        ln.strip(),
                        "body may violate broken ensures",
                    )
    return Validation("discarded-valid", None, None, None, None, "ensures break not manifest")


def first_non_comment_code_line(src: str, needles: tuple[str, ...]) -> str | None:
    for ln in src.splitlines():
        s = ln.strip()
        if not s or s.startswith("//"):
            continue
        if any(n in ln for n in needles):
            return s
    return None


def shared_instead_of_mut_obligation(original: str, mutant: str) -> str | None:
    for ol, ml in zip(original.splitlines(), mutant.splitlines()):
        if "&mut" in ol and "&mut" not in ml and ol.strip() != ml.strip():
            param = re.search(r"(\w+)\s*:", ml)
            name = param.group(1) if param else None
            if name:
                for ln in mutant.splitlines():
                    if name in ln and ("=" in ln or f"&mut {name}" in ln or f".{name}" in ln):
                        if not ln.strip().startswith("//"):
                            return ln.strip()
            return ml.strip()
    return None


def mut_ref_obligation(original: str, mutant: str) -> str | None:
    for ol, ml in zip(original.splitlines(), mutant.splitlines()):
        if "&mut" in ml and "&mut" not in ol and ol.strip() != ml.strip():
            return ml.strip()
    # fallback: first write through new &mut
    for ln in mutant.splitlines():
        if "&mut" in ln and not ln.strip().startswith("//"):
            return ln.strip()
    return None


def classify_shared_mutations(
    kind: str, original: str, mutant: str, omega: str, cur: str
) -> Validation:
    if kind not in ("shared_instead_of_mut", "mut_ref_instead_of_shared"):
        raise ValueError(kind)
    if not current_accepts(omega, cur):
        return Validation("rejected-both", None, None, None, None, "")
    path, heading, quote = SPEC_BY_KIND[kind]
    if kind == "shared_instead_of_mut":
        obl = shared_instead_of_mut_obligation(original, mutant)
        if not obl:
            return Validation("discarded-valid", None, None, None, None, "no write through weakened borrow")
        pm = re.search(r"(\w+)\s*:\s*&\s", obl)
        if pm:
            pname = pm.group(1)
            if not re.search(rf"(?m)^[^/]*\b{re.escape(pname)}\b[^=\n]*=", mutant):
                if f"&mut {pname}" not in mutant and f"{pname}." not in mutant.replace("machine", ""):
                    return Validation(
                        "discarded-valid",
                        None,
                        None,
                        None,
                        obl,
                        "parameter shared borrow only; no body write",
                    )
    else:
        obl = mut_ref_obligation(original, mutant)
        if not obl:
            return Validation("discarded-valid", None, None, None, None, "no illegal &mut introduced")
    if obl.startswith("//"):
        return Validation("discarded-valid", None, None, None, obl, "obligation is comment only")
    return Validation("confirmed-hole", path, quote, heading, obl, "")


def classify_generic(
    kind: str, original: str, mutant: str, omega: str, cur: str, route: str
) -> Validation:
    if kind in ("shared_instead_of_mut", "mut_ref_instead_of_shared"):
        return classify_shared_mutations(kind, original, mutant, omega, cur)
    if kind not in SPEC_BY_KIND:
        return Validation("discarded-no-rule", None, None, None, None, "unknown mutation kind")
    path, heading, quote = SPEC_BY_KIND[kind]
    if not current_accepts(omega, cur):
        return Validation("rejected-both", None, None, None, None, "")
    obl = first_non_comment_code_line(mutant, ("&mut", "[", "/", "="))
    if not obl:
        return Validation("discarded-valid", None, None, None, None, "no concrete obligation line")
    if obl.startswith("//"):
        return Validation("discarded-valid", None, None, None, obl, "obligation is comment only")
    return Validation("confirmed-hole", path, quote, heading, obl, "")


def classify_row(
    kind: str,
    original_path: Path,
    mutant_path: Path,
    omega: str,
    cur: str,
    route: str,
    route_reason: str = "",
) -> Validation:
    kind_report = "remove_mut" if kind == "drop_mut" else kind
    original = original_path.read_text()
    mutant = mutant_path.read_text()

    disagree = (
        omega not in ("skipped", "")
        and omega in ("accepted", "rejected")
        and cur in ("accepted", "rejected")
        and omega != cur
    ) or (
        cur in ("accepted", "rejected")
        and route in ("accepted", "rejected", "incomplete", "fallback")
        and cur != route
    )

    if kind == "drop_requires":
        v = classify_drop_requires(original, mutant, omega, cur, route, route_reason)
    elif kind in ("drop_mut", "remove_mut"):
        v = classify_remove_mut(original, mutant, omega, cur, route)
    elif kind == "break_ensures":
        v = classify_break_ensures(mutant, omega, cur, route)
    elif kind == "drop_ensures":
        return Validation("discarded-valid", None, None, None, None, "weakening only")
    elif kind in ("shared_instead_of_mut", "mut_ref_instead_of_shared"):
        v = classify_shared_mutations(kind, original, mutant, omega, cur)
    elif kind == "widen_requires":
        if not current_accepts(omega, cur):
            return Validation("rejected-both", None, None, None, None, "")
        obl = first_non_comment_code_line(mutant, ("transition", "machine ", "call", "("))
        if not obl:
            return Validation("discarded-valid", None, None, None, None, "no call under widened requires")
        path, heading, quote = SPEC_BY_KIND["widen_requires"]
        if obl.startswith("//"):
        return Validation("discarded-valid", None, None, None, obl, "obligation is comment only")
    return Validation("confirmed-hole", path, quote, heading, obl, "")
    elif kind == "drop_guard":
        if not current_accepts(omega, cur):
            return Validation("rejected-both", None, None, None, None, "")
        obl = first_non_comment_code_line(mutant, ("/", "[", "transition"))
        if not obl or "if " in obl:
            return Validation("discarded-valid", None, None, None, None, "guard removal benign")
        path, heading, quote = SPEC_BY_KIND["drop_guard"]
        if obl.startswith("//"):
        return Validation("discarded-valid", None, None, None, obl, "obligation is comment only")
    return Validation("confirmed-hole", path, quote, heading, obl, "")
    else:
        v = classify_generic(kind_report, original, mutant, omega, cur, route)

    if v.bucket in ("confirmed-hole", "known-gap-q7-mut"):
        return v

    if disagree and v.bucket == "discarded-valid":
        if confirms_invalid_simple(kind_report, original, mutant):
            path, heading, quote = SPEC_BY_KIND.get(
                kind_report, SPEC_BY_KIND.get(kind, ("", "", ""))
            )
            if quote:
                return Validation(
                    "checker-disagreement",
                    path or None,
                    quote or None,
                    heading or None,
                    v.obligation_line,
                    v.notes,
                )

    if v.bucket == "rejected-both":
        return v
    if not current_accepts(omega, cur) and not disagree:
        return Validation("rejected-both", None, None, None, None, "")
    if route == "accepted" and not current_accepts(omega, cur):
        return Validation("discarded-valid", None, None, None, None, "route-only accept")
    return v


def confirms_invalid_simple(kind: str, original: str, mutant: str) -> bool:
    if kind in ("remove_mut", "drop_mut"):
        name = first_dropped_mut_binding(original, mutant)
        return bool(name and binding_write_obligation(name, mutant))
    if kind == "drop_requires":
        v = classify_drop_requires(original, mutant, "accepted", "accepted", "rejected")
        return v.bucket == "confirmed-hole"
    return mutant != original
