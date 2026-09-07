"""Literal comparison requests and independently specified exact observations."""

import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent.parent / "derivation-layout"))

import boundaries
import forwarding
import roots
import sessions


ENTRIES = ("root", "session", "retention", "witness", "invalid", "budget", "resume", "pending")


def cases():
    for group in (roots, sessions, forwarding, boundaries):
        yield from group.cases()
