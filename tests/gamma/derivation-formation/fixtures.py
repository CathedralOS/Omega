"""Concept-owned literal fixtures; no decoding or semantic interpretation."""

import sys
from pathlib import Path

# Reuse only the neighboring layout gate's literal wire encoder.
sys.path.append(str(Path(__file__).resolve().parent.parent / "derivation-layout"))

import applications
import clauses
import decrease
import forwarding
import positive
import resources
import signatures


def cases():
    for group in (positive, signatures, clauses, applications, decrease, forwarding, resources):
        yield from group.cases()
