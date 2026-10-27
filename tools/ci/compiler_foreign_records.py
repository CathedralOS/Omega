#!/usr/bin/env python3
"""Matching-host C scalar-record result evidence through the normal corpus gate."""

import argparse
from contextlib import contextmanager, nullcontext
import ctypes
import hashlib
import json
import os
from pathlib import Path
import sys
import shutil
import subprocess
import time
import tempfile

import compiler_fact_rows

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

# A child retains every parent membership and its independent issuer group.
POSITIVES += tuple(f"pass/facts/opaque_independent_inherited_result_through_{p}_exit" for p in PLACES)
NEGATIVES += tuple(f"fail/facts/opaque_independent_inherited_result_through_{p}_unissued" for p in PLACES)
POSITIVES += ("pass/facts/opaque_inherited_result_scalar_parent_tag_exit",
              "pass/domains/domain_pattern_match_bound_inherited_routed_retained_exit",
              "pass/domains/domain_pattern_match_bound_alias_routed_retained_exit")
NEGATIVES += ("fail/domains/domain_pattern_match_bound_inherited_routed_unproven_rejected",)


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
    retained = cli.resolve()
    candidate = compiler_pilot.command(["git", "rev-parse", "HEAD"])
    (output / "compiler-profile.json").write_text(json.dumps(dict(
        observation=profile, source_revision=candidate), indent=2) + "\n", encoding="utf-8")
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
        for family, name in ((family, name) for family in ("", "refinement")
                             for name in ("raw_native_result", "component_launders_raw_result")):
            authored = compiler_pilot.ROOT / "samples/cli/facts/opaque_closed_carrier_exchange/independent_qualified_result" / family / "controls" / name
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
                stem = "-".join(part for part in (family, name, mode) if part)
                (output / (stem + ".stdout.txt")).write_text(result.stdout, encoding="utf-8")
                (output / (stem + ".stderr.txt")).write_text(result.stderr, encoding="utf-8")
                passed = result.returncode == code and (mode == "check" or all(x in result.stdout + result.stderr for x in expected))
                records.append(dict(family=family or "direct", control=name, mode=mode, argv=arguments, exit=result.returncode,
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


OWNED_ROWS = {
    "nested": ("foreign_scalar_record_result/nested", "foreign_nested_scalar_record_result"),
    "arrays": ("foreign_scalar_record_result/arrays", "foreign_owned_array_record"),
}
COHORTS = ("scalar-opaque",) + tuple(OWNED_ROWS)


def select_cohorts(selection):
    selected = COHORTS if selection == "all" else tuple(dict.fromkeys(
        part.strip() for part in selection.split(",") if part.strip()))
    if not selected or any(name not in COHORTS for name in selected):
        raise ValueError("cohorts must name scalar-opaque, nested, arrays, or all")
    return selected


def select_fixtures(selection, inventories):
    """Select exact catalog IDs; a diagnostic subset never qualifies a full row."""
    if selection is None:
        return inventories
    requested = tuple(part.strip() for part in selection.split(","))
    if not requested or any(not part for part in requested):
        raise ValueError("fixtures must name a nonempty comma-separated list of exact catalog IDs")
    known = {fixture for positive, negative in inventories.values()
             for fixture in (*positive, *negative)}
    unknown = set(requested) - known
    if unknown:
        raise ValueError(f"unknown fixtures in selected cohorts: {sorted(unknown)}")
    wanted = set(requested)
    selected = {name: (tuple(f for f in positive if f in wanted),
                       tuple(f for f in negative if f in wanted))
                for name, (positive, negative) in inventories.items()}
    return {name: pair for name, pair in selected.items() if any(pair)}


@contextmanager
def provider_path(library):
    key = "PATH" if os.name == "nt" else "DYLD_LIBRARY_PATH" if sys.platform == "darwin" else "LD_LIBRARY_PATH"
    previous = os.environ.get(key)
    os.environ[key] = str(library.parent) + (os.pathsep + previous if previous else "")
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = previous


def prepare_provider(fact, output):
    """Authenticate the real matching-host C fixture before a compiler build."""
    output.mkdir(parents=True, exist_ok=False)
    recipe = fact / "fixture/prepare.py"
    arguments = [sys.executable, str(recipe), "--output", str(output)]
    result = subprocess.run(arguments, cwd=compiler_pilot.ROOT, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=180)
    (output / "prepare.stdout.json").write_text(result.stdout, encoding="utf-8")
    (output / "prepare.stderr.txt").write_text(result.stderr, encoding="utf-8")
    if result.returncode:
        raise ValueError(f"C fixture failed ({result.returncode}): {result.stderr or result.stdout}")
    receipt = json.loads(result.stdout)
    library = Path(receipt["library"]).resolve()
    selftest = Path(receipt["commands"][-1][0]).resolve()
    if library.parent != output.resolve() or selftest.parent != output.resolve() or receipt["selftest_exit"] != 0:
        raise ValueError("C fixture did not retain its matching-host library and successful self-test")
    for path, expected in ((library, receipt["library_sha256"]),
                           (selftest, receipt["selftest_sha256"]),
                           (fact / "fixture/provider.c", receipt["provider_sha256"]),
                           (fact / "fixture/provider.map", receipt["map_sha256"])):
        if compiler_fact_rows.digest(path) != expected:
            raise ValueError(f"C fixture resource digest differs: {path}")
    receipt.update(recipe_sha256=compiler_fact_rows.digest(recipe), preparation_argv=arguments,
                   preparation_exit=result.returncode)
    (output / "provider.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return library


def scalar_opaque(output, producer):
    authenticate_windows_lldiv(output)
    status = compiler_pilot.run_suite(str(output.relative_to(compiler_pilot.ROOT / "build")),
                                     POSITIVES, NEGATIVES, producer=producer)
    # Preserve public controls even when an independent corpus control failed.
    failures = []
    (output / "public-cli").mkdir(parents=True, exist_ok=True)
    for operation in (qualify_nested_source_import, qualify_result_provenance):
        try:
            operation(output / "public-cli", producer.cli)
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            failures.append(str(error))
    if status or failures:
        raise ValueError(f"scalar/opaque corpus exit {status}; public controls: {failures}")


def owned_row(output, fact, prefix, library, producer):
    positives, negatives = compiler_fact_rows.fixtures(compiler_pilot.ROOT, fact, prefix)
    resources = {str(path): compiler_fact_rows.digest(path)
                 for folder in (output / "provider", fact / "fixture")
                 for path in folder.iterdir() if path.is_file()}
    (output / "resource-inputs.json").write_text(json.dumps(resources, indent=2) + "\n", encoding="utf-8")
    with provider_path(library):
        inventory = compiler_fact_rows.prepare(output, fact, producer)
        status = compiler_pilot.run_suite(str(output.relative_to(compiler_pilot.ROOT / "build")),
                                         positives, negatives, producer=producer)
        try:
            compiler_fact_rows.measure(output, inventory, producer)
        finally:
            for path, expected in resources.items():
                if compiler_fact_rows.digest(path) != expected:
                    raise ValueError(f"provider resource changed during measurement: {path}")
        if status:
            raise ValueError(f"owned row corpus failed (exit {status}); public observations retained")


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cohorts", default=os.environ.get("OMEGA_FOREIGN_RECORD_COHORTS") or "scalar-opaque",
                        help="comma-separated scalar-opaque,nested,arrays; all selects every cohort")
    parser.add_argument("--fixtures", default=os.environ.get("OMEGA_FOREIGN_RECORD_FIXTURES") or None,
                        help="optional comma-separated exact catalog IDs; diagnostic corpus subset only")
    parser.add_argument("--list", action="store_true", help="show exact selected fixtures without building")
    options = parser.parse_args(arguments)
    output = compiler_pilot.ROOT / "build/compiler-foreign-records"
    selected = select_cohorts(options.cohorts)
    plans = {}
    for name in selected:
        if name in OWNED_ROWS:
            relative, prefix = OWNED_ROWS[name]
            fact = compiler_pilot.ROOT / "samples/cli/facts" / relative
            plans[name] = (fact, prefix, *compiler_fact_rows.fixtures(compiler_pilot.ROOT, fact, prefix))
    inventories = select_fixtures(options.fixtures, {
        name: plans[name][2:] if name in plans else (POSITIVES, NEGATIVES)
        for name in selected})
    scope = "selected-corpus-fixtures" if options.fixtures is not None else "full-cohorts"
    if options.list:
        print(json.dumps({name: dict(positive=pair[0], negative=pair[1])
                          for name, pair in inventories.items()}, indent=2))
        return 0
    os.environ["OMEGA_LIBRARY_ROOT"] = str(compiler_pilot.ROOT / "source/library")
    output.mkdir(parents=True, exist_ok=True)
    attempt = Path(tempfile.mkdtemp(prefix="attempt-", dir=output))
    report = dict(candidate=compiler_pilot.command(["git", "rev-parse", "HEAD"]),
                  target=compiler_pilot.corpus_gate.host_target(), scope=scope, cohorts=[], passed=False)
    try:
        ready = []
        for name, (positives, negatives) in inventories.items():
            record = dict(name=name, scope=scope, positive=positives, negative=negatives, passed=False)
            report["cohorts"].append(record)
            cohort = attempt / name
            cohort.mkdir()
            try:
                library = prepare_provider(plans[name][0], cohort / "provider") if name in plans else None
                ready.append((name, cohort, library, record))
            except (OSError, ValueError, subprocess.SubprocessError) as error:
                record["error"] = str(error)
        if ready:
            producer = compiler_pilot.prepare_producer(attempt)
            report["producer"] = producer.record()
            for name, cohort, library, record in ready:
                try:
                    if options.fixtures is not None:
                        with provider_path(library) if library is not None else nullcontext():
                            status = compiler_pilot.run_suite(
                                str(cohort.relative_to(compiler_pilot.ROOT / "build")),
                                inventories[name][0], inventories[name][1], producer=producer)
                        if status:
                            raise ValueError(f"selected corpus fixtures failed (exit {status})")
                    elif name in plans:
                        owned_row(cohort, plans[name][0], plans[name][1], library, producer)
                    else:
                        scalar_opaque(cohort, producer)
                    record["passed"] = True
                except (OSError, ValueError, subprocess.SubprocessError) as error:
                    record["error"] = str(error)
                print(f"foreign records: {name}: {record}", flush=True)
        report["passed"] = bool(report["cohorts"]) and all(row["passed"] for row in report["cohorts"])
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        report["error"] = str(error)
    finally:
        (attempt / "summary.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        (output / "latest.json").write_text(json.dumps(dict(attempt=str(attempt), **report), indent=2) + "\n", encoding="utf-8")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"foreign records: {error}", file=sys.stderr)
        sys.exit(1)
