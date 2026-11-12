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
