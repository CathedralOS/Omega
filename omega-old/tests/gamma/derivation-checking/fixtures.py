"""Literal proof records and independently specified checked outcomes."""

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent.parent / "derivation-layout"))

import forwarding
import positive
import references
import relations
import resources
import roots
import unfolding


def cases():
    for group in (positive, references, relations, unfolding, roots, forwarding, resources):
        yield from group.cases()
