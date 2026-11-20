"""Run the intended verdicts of the published front-end examples.

Run from any directory with the repository's qualified mbx on PATH:
    python tools/spec-corpus/check_frontend_readiness.py
Exit zero requires every named case to run and pass. This is independent of
the isolated fragment snapshots and of downstream compiler qualification.
"""

from collections import Counter
from pathlib import Path
import subprocess
import sys


WORKSPACE = Path(__file__).absolute().parents[2]
STAGE_TESTS = {
    "language/domains.md:1020": (
        "published_quantity_keeps_its_carrier_and_unit_index",
        "explicit_quantity_carrier_matches_the_subject_in_every_domain_position",
    ),
    "language/effects.md:51": ("published_inherited_call_contributes_its_declared_reach",),
    "language/effects.md:203": ("published_counter_fields_retain_their_callable_contracts",),
    "language/effects.md:878": ("published_pause_markers_cover_each_required_effect",),
    "language/generics.md:61": ("published_sort_requirement_binds_each_calls_element_and_ordering",),
    "language/generics.md:310": (
        "published_equality_binds_a_shadowing_parameter",
        "published_static_type_disjunction_reaches_typing",
    ),
    "language/generics.md:640": ("published_static_parameters_require_the_declared_argument_role",),
    "language/machine_values.md:22": ("published_record_call_selects_the_shared_counter_field",),
    "language/machine_values.md:44": ("published_reader_and_factory_bind_their_own_preconditions",),
    "language/modules.md:117": ("published_import_exposes_the_exact_remote_domain",),
}
PRODUCT_CASES = {
    "language/conformances.md:204": (
        "encode and decode retain their selected requirements",
        r"reject lib_b\decode in the lib_a\encode slot",
        r"reject lib_a\encode in the lib_b\decode slot",
    ),
    "language/reflection.md:77": (
        r"reflect\field(Controller::status)", r"reject reflect\callable(Controller::status)",
        r"reflect\field(Buffer::<u32>::used)", r"reject reflect\callable(Buffer::<u32>::used)",
        r"reflect\callable(Controller\update)", r"reject reflect\field(Controller\update)",
        r"reflect\callable(ControllerWriter::write)", r"reject reflect\field(ControllerWriter::write)",
    ),
    "language/some_packages.md:41": (
        "one File witness supplies both record fields",
        "refuse a callable over Socket at the initializer",
    ),
}


def context_results(label, lines):
    """A section must contain each positive and negative check once before its verdict."""
    verdicts = [i for i, line in enumerate(lines) if line in {"PASS " + label, "FAIL " + label}]
    starts = [i for i, line in enumerate(lines) if line == label]
    if len(starts) != 1 or len(verdicts) != 1 or verdicts[0] <= starts[0]:
        return False, [], f"expected one complete context section for {label}"
    section = lines[starts[0] + 1:verdicts[0]]
    details = [line.strip() for line in section if line.startswith(("  PASS ", "  FAIL "))]
    names = Counter(line.split(" ", 1)[1].partition(" — ")[0] for line in details)
    required = Counter(PRODUCT_CASES[label])
    if names != required or any(line in PRODUCT_CASES for line in section):
        return False, details, (f"{label}: missing {list((required - names).elements())}, "
                                f"extra {list((names - required).elements())} or overlapping context sections")
    passed = all(line.startswith("PASS ") for line in details)
    if passed != (lines[verdicts[0]] == "PASS " + label):
        return False, details, f"{label}: context verdict disagrees with the individual witnesses"
    return passed, details, None


def run_mbx(*arguments):
    completed = subprocess.run(
        ["mbx", *arguments],
        cwd=WORKSPACE,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    print(completed.stdout, end="")
    return completed.returncode, completed.stdout.splitlines()


def main():
    # Run the integration targets without a name filter. Losing a required
    # test must keep its example out of the verified count.
    tests_code, tests = run_mbx(
        "test", "--locked", "--no-fail-fast", "-p", "omega-resolver", "-p", "omega-typer",
        "--test", "stage", "--test", "contextual_examples", "--", "--format", "pretty",
    )
    products_code, products = run_mbx(
        "run", "--locked", "-p", "spec-corpus", "--features", "readiness", "--bin", "published_readiness",
    )
    verdicts = {
        label: all(f"test {name} ... ok" in tests for name in names)
        for label, names in STAGE_TESTS.items()
    }
    for label in PRODUCT_CASES:
        passed, _, error = context_results(label, products)
        verdicts[label] = passed
        if error:
            print(error)
    print("\nFront-end verdicts")
    for label, passed in sorted(verdicts.items()):
        print(f"{'PASS' if passed else 'UNVERIFIED'} {label}")
    print(f"{sum(verdicts.values())}/{len(verdicts)} intended verdicts verified")
    return int(bool(tests_code or products_code or not all(verdicts.values())))


if __name__ == "__main__":
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(errors="backslashreplace")
    sys.exit(main())
