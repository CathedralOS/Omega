#!/usr/bin/env python3
"""Matching-host C scalar-record result evidence through the normal corpus gate."""

import ctypes
import hashlib
import json
import os
from pathlib import Path
import sys

import compiler_pilot

PLACES = ("local", "field", "parameter", "call_result", "match_join", "const_name",
          "other_state", "other_machine", "target_body")
POSITIVES = tuple(f"pass/facts/foreign_scalar_record_result_through_{p}_exit" for p in PLACES)
NEGATIVES = tuple(f"fail/facts/foreign_scalar_record_result_through_{p}_rejected" for p in PLACES)
POSITIVES += tuple(f"pass/facts/opaque_independent_copy_through_{p}_exit" for p in PLACES)
NEGATIVES += tuple(f"fail/facts/opaque_independent_copy_through_{p}_rejected" for p in PLACES)
# Copy support must preserve the existing take-once component lane as well.
POSITIVES += tuple(f"pass/facts/opaque_independent_closed_carrier_through_{p}_exit" for p in PLACES)
NEGATIVES += tuple(f"fail/facts/opaque_independent_closed_carrier_through_{p}_cleanup" for p in PLACES)
# Linear transport carries one actual whole claim across the same component edge.
POSITIVES += tuple(f"pass/facts/opaque_independent_linear_through_{p}_exit" for p in PLACES)
NEGATIVES += tuple(f"fail/facts/opaque_independent_linear_through_{p}_unconsumed" for p in PLACES)


def authenticate_windows_lldiv(output):
    """Observe the SDK's exact PE import, not a guessed library or fixture result.

    The Microsoft SDK ucrt.lib imports this API-set export; its forwarding DLL
    resolves to ucrtbase.lldiv. Native emitted code uses the same literal import.
    ctypes independently calls that real two-i64 C return on the matching host.
    """
    if sys.platform != "win32":
        return

    class Division(ctypes.Structure):
        _fields_ = [("quotient", ctypes.c_longlong), ("remainder", ctypes.c_longlong)]

    library = ctypes.CDLL("api-ms-win-crt-utility-l1-1-0.dll")
    operation = library.lldiv
    operation.argtypes = [ctypes.c_longlong, ctypes.c_longlong]
    operation.restype = Division
    if ctypes.sizeof(Division) != 16 or ctypes.alignment(Division) != 8:
        raise ValueError("Windows lldiv C result geometry differs")
    observed = []
    for numerator, denominator, expected in (
            (-70000000000, 3, (-23333333333, -1)),
            (70000000000, 9, (7777777777, 7))):
        actual = operation(numerator, denominator)
        fields = (actual.quotient, actual.remainder)
        if fields != expected:
            raise ValueError("actual Windows lldiv result differs")
        observed.append(dict(numerator=numerator, denominator=denominator,
                             quotient=actual.quotient, remainder=actual.remainder))
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetModuleFileNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_uint]
    kernel.GetModuleFileNameW.restype = ctypes.c_uint
    path = ctypes.create_unicode_buffer(32768)
    length = kernel.GetModuleFileNameW(library._handle, path, len(path))
    if length == 0 or length >= len(path):
        raise ValueError("cannot authenticate the loaded Windows C library")
    binary = Path(path.value)
    output.mkdir(parents=True, exist_ok=True)
    (output / "windows-lldiv.json").write_text(json.dumps(dict(
        schema="omega-windows-lldiv-host-observation-v1",
        candidate=compiler_pilot.command(["git", "rev-parse", "HEAD"]),
        library="api-ms-win-crt-utility-l1-1-0.dll", export="lldiv",
        loaded_image=str(binary), sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        result_size=ctypes.sizeof(Division), result_alignment=ctypes.alignment(Division),
        observations=observed), indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    os.environ["OMEGA_LIBRARY_ROOT"] = str(compiler_pilot.ROOT / "source/library")
    authenticate_windows_lldiv(compiler_pilot.ROOT / "build/compiler-foreign-records")
    sys.exit(compiler_pilot.run_suite("compiler-foreign-records", POSITIVES, NEGATIVES))
