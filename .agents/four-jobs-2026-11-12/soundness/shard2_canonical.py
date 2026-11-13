#!/usr/bin/env python3
"""Canonical mutation kinds (shard 2 run 2)."""
from __future__ import annotations

CANONICAL_KINDS = (
    "drop_requires",
    "widen_requires",
    "remove_guard_transition",
    "remove_mut",
    "mut_ref_instead_of_shared",
    "shared_instead_of_mut",
    "none",
)

LEGACY_MAP = {
    "drop_machine_requires": "drop_requires",
    "drop_requires_body": "drop_requires",
    "widen_requires_lt": "widen_requires",
    "delete_first_if_guard": "remove_guard_transition",
    "remove_mut": "remove_mut",
    "ref_to_mut_ref": "mut_ref_instead_of_shared",
    "mut_ref_to_ref": "shared_instead_of_mut",
    "drop_machine_ensures": "none",
}


def canonical_kind(legacy: str) -> str:
    return LEGACY_MAP.get(legacy, legacy if legacy in CANONICAL_KINDS else "none")
