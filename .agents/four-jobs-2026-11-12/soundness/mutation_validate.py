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


def classify_drop_requires(
    original: str, mutant: str, omega: str, cur: str, route: str
) -> Validation:
    removed: list[str] = []
    for m in REQUIRES_RE.finditer(original):
        clause = m.group(1).strip()
        if clause not in mutant:
            removed.append(clause)
    if not removed:
        return Validation("discarded-valid", None, None, None, None, "no removed requires")

    # Map removed clause to machine body that lost it.
    for clause in removed:
        idx = original.find(f"requires {clause}")
        if idx < 0:
            continue
        body_start = original.find("{", idx)
        if body_start < 0:
            continue
        depth = 0
        body_end = None
        for i, ch in enumerate(original[body_start:], start=body_start):
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    body_end = i
                    break
        if body_end is None:
            continue
        body = original[body_start + 1 : body_end]
        for line in body.splitlines():
            ls = line.strip()
            if not ls or ls.startswith("//"):
                continue
            if "[" in ls or "/" in ls or re.search(r"\b\w+\s*\(", ls):
                tokens = re.findall(r"\b([A-Za-z_][A-Za-z0-9_.\[\]]+)\b", clause)
                if any(t.split(".")[0] in ls for t in tokens):
                    if clause in body:
                        return Validation(
                            "discarded-valid",
                            None,
                            None,
                            None,
                            None,
                            "requires fact duplicated in body",
                        )
                    if current_accepts(omega, cur):
                        return Validation(
                            "confirmed-hole",
                            "proofs/contracts.md",
                            "Before importing a callee's conclusions, a caller establishes the assumptions under the exact argument substitution.",
                            "## Machines and bundles",
                            ls,
                            f"dropped requires: {clause}",
                        )
    return Validation(
        "discarded-valid",
        None,
        None,
        None,
        None,
        "dropped requires not tied to a body obligation",
    )


def first_dropped_mut_binding(original: str, mutant: str) -> str | None:
    orig_pairs = [(m.group(1) is not None, m.group(2)) for m in LET_BIND_RE.finditer(original)]
    mut_pairs = [(m.group(1) is not None, m.group(2)) for m in LET_BIND_RE.finditer(mutant)]
    for (om, on), (mm, mn) in zip(orig_pairs, mut_pairs):
        if om and not mm and on == mn:
            return on
    return None


def binding_write_obligation(name: str, src: str) -> str | None:
    """Return a source line showing illegal write/&mut on a non-mut binding (no shadowing)."""
    lines = src.splitlines()
    declared: dict[str, bool] = {}
    for ln in lines:
        m = LET_BIND_RE.search(ln)
        if m:
            declared[m.group(2)] = m.group(1) is not None
        stripped = ln.strip()
        if stripped.startswith("//"):
            continue
        if re.search(rf"\b{re.escape(name)}\s*=", ln) and not declared.get(name, True):
            return stripped
        if re.search(rf"&mut\s+{re.escape(name)}\b", ln):
            return stripped
        if re.search(rf"{re.escape(name)}\.", ln) and "=" in ln and not declared.get(name, True):
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
    return Validation("confirmed-hole", path, quote, heading, obl, "")


def classify_row(
    kind: str,
    original_path: Path,
    mutant_path: Path,
    omega: str,
    cur: str,
    route: str,
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
        v = classify_drop_requires(original, mutant, omega, cur, route)
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
        return Validation("confirmed-hole", path, quote, heading, obl, "")
    elif kind == "drop_guard":
        if not current_accepts(omega, cur):
            return Validation("rejected-both", None, None, None, None, "")
        obl = first_non_comment_code_line(mutant, ("/", "[", "transition"))
        if not obl or "if " in obl:
            return Validation("discarded-valid", None, None, None, None, "guard removal benign")
        path, heading, quote = SPEC_BY_KIND["drop_guard"]
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
