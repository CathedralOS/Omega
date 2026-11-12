"""Exact wiki/spec excerpts for shard soundness review (file path + quoted text)."""

from __future__ import annotations

SPEC_QUOTES: dict[str, tuple[str, str]] = {
    "drop_mut": (
        "language/ownership.md",
        "modifying an established ordinary local's owned storage requires a `mut` binding. "
        "This covers direct assignment, writes to owned fields or array elements, and "
        "formation of `&mut` or `&write` loans to those places.",
    ),
    "shared_instead_of_mut": (
        "language/ownership.md",
        "modifying an established ordinary local's owned storage requires a `mut` binding. "
        "This covers direct assignment, writes to owned fields or array elements, and "
        "formation of `&mut` or `&write` loans to those places.",
    ),
    "mut_ref_instead_of_shared": (
        "language/ownership.md",
        "A callee cannot grant writable access that the caller's place lacks.",
    ),
    "drop_requires": (
        "proofs/contracts.md",
        "Ordinary calls also follow local call summaries: an exact checked call in the same "
        "fixed verification unit may expose additional checked body-derived facts. Outside that "
        "unit, additional semantic guarantees must be authored in the published contract.",
    ),
    "widen_requires": (
        "language/state_contracts.md",
        "Machine | Entry obligations established at every invocation under exact actual-argument substitution.",
    ),
    "drop_ensures": (
        "proofs/contracts.md",
        "`ensures` states conclusions its body must justify.",
    ),
    "break_ensures": (
        "proofs/contracts.md",
        "A written `ensures` remains an obligation and a minimum guarantee; local precision "
        "neither rewrites it nor strengthens an abstract requirement.",
    ),
    "drop_guard": (
        "language/state_contracts.md",
        "Repeated predicate clauses in one scope conjoin their obligations.",
    ),
}
