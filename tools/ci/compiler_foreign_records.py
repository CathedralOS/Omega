#!/usr/bin/env python3
"""Matching-host C scalar-record result evidence through the normal corpus gate."""

import ctypes
import hashlib
import json
import os
from pathlib import Path
import sys
import shutil
import subprocess
import time

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

# Whole routed membership follows the actual owned input into its installed child.
POSITIVES += tuple(f"pass/facts/opaque_independent_qualified_input_through_{p}_exit" for p in PLACES)
NEGATIVES += tuple(f"fail/facts/opaque_independent_qualified_input_through_{p}_unissued" for p in PLACES)
POSITIVES += ("pass/facts/opaque_independent_qualified_input_import_alias_exit",
              "pass/facts/opaque_independent_qualified_input_renumbered_catalog_exit")
NEGATIVES += ("fail/facts/opaque_independent_qualified_input_bare_sibling_unissued",)

# Checked independent results carry existing other-issuer membership back out.
POSITIVES += tuple(f"pass/facts/opaque_independent_qualified_result_through_{p}_exit" for p in PLACES)
NEGATIVES += tuple(f"fail/facts/opaque_independent_qualified_result_through_{p}_unissued" for p in PLACES)
POSITIVES += ("pass/facts/opaque_independent_qualified_result_import_alias_exit",
              "pass/facts/opaque_independent_qualified_result_renumbered_catalog_exit")


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


def qualify_nested_source_import(output, cli):
    """Exercise captured nested ownership through the actual public CLI.

    Corpus preparation relocates dependencies to disjoint sealed roots. Its
    parent-owned local copy does not preserve this authored ownership boundary.
    Keep the exact nested hierarchy and the intended rejection here instead.
    """
    root = compiler_pilot.ROOT
    authored = root / "samples/cli/facts/opaque_closed_carrier_exchange/independent_qualified/controls/undeclared_nested_import"
    profile = compiler_pilot.corpus_gate.require_optimized(cli)
    digest = hashlib.sha256(cli.read_bytes()).hexdigest()
    output.mkdir(parents=True, exist_ok=True)
    retained = output / cli.name
    shutil.copy2(cli, retained)
    candidate = compiler_pilot.command(["git", "rev-parse", "HEAD"])
    Path(str(retained) + ".compiler-profile.json").write_text(json.dumps(dict(
        profile, binary_sha256=digest, source_revision=candidate), indent=2) + "\n", encoding="utf-8")
    def source_hashes():
        return {str(path.relative_to(authored)): hashlib.sha256(path.read_bytes()).hexdigest()
                for path in authored.rglob("*")
                if path.is_file() and ".omega" not in path.relative_to(authored).parts
                and path.suffix in (".omg", ".txt")}

    sources = source_hashes()
    arguments = [str(retained), "--check", str(authored / "main.omg")]
    started = time.monotonic()
    result = subprocess.run(arguments, cwd=root, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=180)
    (output / "nested-import.stdout.txt").write_text(result.stdout, encoding="utf-8")
    (output / "nested-import.stderr.txt").write_text(result.stderr, encoding="utf-8")
    expected = [line.strip() for line in (authored / "expected.txt").read_text(encoding="utf-8").splitlines()
                if line.strip() and not line.startswith("#")]
    passed = (result.returncode == 1 and bool(expected)
              and all(line in result.stdout + result.stderr for line in expected)
              and source_hashes() == sources
              and hashlib.sha256(retained.read_bytes()).hexdigest() == digest)
    observation = dict(candidate=candidate, target=compiler_pilot.corpus_gate.host_target(),
                       argv=arguments, exit=result.returncode, seconds=time.monotonic() - started,
                       compiler_sha256=digest, compiler_profile=profile,
                       validator_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                       dirty=bool(compiler_pilot.command(["git", "status", "--porcelain"])),
                       source_sha256=sources, expected=expected, passed=passed)
    (output / "nested-import.json").write_text(json.dumps(observation, indent=2) + "\n", encoding="utf-8")
    if not passed:
        raise ValueError(f"public CLI nested import did not reject its intended ownership boundary: {result.stderr}")
    return observation


def qualify_result_provenance(output, cli):
    """A checked component cannot launder another issuer's raw-host promise."""
    profile = compiler_pilot.corpus_gate.require_optimized(cli)
    digest = hashlib.sha256(cli.read_bytes()).hexdigest()
    candidate = compiler_pilot.command(["git", "rev-parse", "HEAD"])
    records = []
    report = dict(candidate=candidate, target=compiler_pilot.corpus_gate.host_target(),
                  compiler_sha256=digest, compiler_profile=profile, observations=records,
                  dirty=bool(compiler_pilot.command(["git", "status", "--porcelain"])), passed=False)
    try:
        for name in ("raw_native_result", "component_launders_raw_result"):
            authored = compiler_pilot.ROOT / "samples/cli/facts/opaque_closed_carrier_exchange/independent_qualified_result/controls" / name
            def source_hashes():
                return {str(p.relative_to(authored)): hashlib.sha256(p.read_bytes()).hexdigest()
                        for p in authored.rglob("*") if p.is_file()
                        and ".omega" not in p.relative_to(authored).parts
                        and p.suffix in (".omg", ".txt")}
            sources = source_hashes()
            expected = [line.strip() for line in (authored / "expected_native_error.txt").read_text(encoding="utf-8").splitlines()
                        if line.strip() and not line.startswith("#")]
            for mode, flags, code in (("check", ("--check",), 0), ("native", ("run", "--both"), 200)):
                arguments = [str(cli), *flags, str(authored / "main.omg")]
                result = subprocess.run(arguments, cwd=compiler_pilot.ROOT, stdin=subprocess.DEVNULL,
                                        capture_output=True, text=True, timeout=180)
                stem = name + "-" + mode
                (output / (stem + ".stdout.txt")).write_text(result.stdout, encoding="utf-8")
                (output / (stem + ".stderr.txt")).write_text(result.stderr, encoding="utf-8")
                passed = result.returncode == code and (mode == "check" or all(x in result.stdout + result.stderr for x in expected))
                records.append(dict(control=name, mode=mode, argv=arguments, exit=result.returncode,
                                    expected=expected if mode == "native" else [], source_sha256=sources, passed=passed))
                if not passed:
                    raise ValueError(f"{name}/{mode}: missing intended provenance admission rejection: {result.stderr}")
            if source_hashes() != sources:
                raise ValueError(f"{name}: authored source changed during control execution")
        if hashlib.sha256(cli.read_bytes()).hexdigest() != digest:
            raise ValueError("provenance-control compiler changed during execution")
        report["passed"] = True
    finally:
        (output / "result-provenance.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    os.environ["OMEGA_LIBRARY_ROOT"] = str(compiler_pilot.ROOT / "source/library")
    output = compiler_pilot.ROOT / "build/compiler-foreign-records"
    authenticate_windows_lldiv(output)
    status = compiler_pilot.run_suite("compiler-foreign-records", POSITIVES, NEGATIVES)
    name = "omega.exe" if os.name == "nt" else "omega"
    cli = Path(os.environ.get("CARGO_TARGET_DIR", compiler_pilot.ROOT / "target")) / "debug" / name
    try:
        qualify_nested_source_import(output / "public-cli", cli.resolve())
        qualify_result_provenance(output / "public-cli", (output / "public-cli" / name).resolve())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"foreign records public CLI: {error}", file=sys.stderr)
        status = 1
    sys.exit(status)
