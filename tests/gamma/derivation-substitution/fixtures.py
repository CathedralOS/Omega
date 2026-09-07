"""Explicit source-owned diagnostics, literal wire records, authored outcomes."""

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent.parent / "derivation-layout"))

import rejections
import resources
import sessions
import unfolding

ENTRIES = ("root", "case", "clause", "invalid", "session", "retention", "bulk", "budget", "witness")


def cases():
    for group in (unfolding, sessions, rejections, resources):
        yield from group.cases()
