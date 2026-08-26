#!/usr/bin/env python3
"""Exact conservative CKIR15 ELF reconstruction adapter."""

from __future__ import annotations

from omgrfn16_elf_reference import Reconstructor
from omgrfn17_ckir import decode
from omgrfn17_frame import RefinementError, RefinementResourceError


def reconstruct(contents: bytes) -> bytes:
    try:
        return Reconstructor(decode(contents)).reconstruct()
    except RefinementResourceError:
        raise
    except Exception as error:
        raise RefinementError(f"exact CKIR15 ELF reconstruction: {error}") from error
