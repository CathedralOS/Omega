"""Explicit ground fixtures and expected observations, with no semantic parser."""

import sys
from pathlib import Path

# The layout gate owns the shared literal field encoder, not a host decoder.
sys.path.append(str(Path(__file__).resolve().parent.parent / "derivation-layout"))

import applications
import forwarding
import large
import positive
import references
import roots


def cases():
    for group in (positive, references, applications, roots, forwarding, large):
        yield from group.cases()
