"""Run the intended verdicts of the twelve published V2 front-end examples.

Run from any directory with the repository's qualified mbx on PATH:
    python tools/spec-corpus/check_frontend_readiness.py
Exit zero requires every named case to run and pass. This is independent of
the isolated fragment snapshots and of downstream compiler qualification.
"""

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
    "language/effects.md:201": ("published_counter_fields_retain_their_callable_contracts",),
    "language/effects.md:871": ("published_pause_markers_cover_each_required_effect",),
    "language/generics.md:61": ("published_sort_requirement_binds_each_calls_element_and_ordering",),
    "language/generics.md:306": (
        "published_equality_binds_a_shadowing_parameter",
        "published_static_type_disjunction_reaches_typing",
    ),
    "language/generics.md:636": ("published_static_parameters_require_the_declared_argument_role",),
    "language/machine_values.md:17": ("published_record_call_selects_the_shared_counter_field",),
    "language/modules.md:102": ("published_import_exposes_the_exact_remote_domain",),
}
PRODUCT_CASES = (
    "language/conformances.md:204",
    "language/reflection.md:77",
    "language/some_packages.md:39",
)


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
    return completed.returncode, set(completed.stdout.splitlines())


def main():
    # Run the integration targets without a name filter. Losing a required
    # test must keep its example out of the verified count.
    tests_code, tests = run_mbx(
        "test", "--locked", "--no-fail-fast", "-p", "omega-resolver", "-p", "omega-typer",
        "--test", "stage", "--test", "contextual_examples", "--", "--format", "pretty",
    )
    products_code, products = run_mbx(
        "run", "--locked", "-p", "omega-typer", "--example", "published_readiness",
    )
    verdicts = {
        label: all(f"test {name} ... ok" in tests for name in names)
        for label, names in STAGE_TESTS.items()
    }
    verdicts.update({label: f"PASS {label}" in products for label in PRODUCT_CASES})
    print("\nFront-end verdicts")
    for label, passed in sorted(verdicts.items()):
        print(f"{'PASS' if passed else 'UNVERIFIED'} {label}")
    print(f"{sum(verdicts.values())}/{len(verdicts)} intended verdicts verified")
    return int(bool(tests_code or products_code or not all(verdicts.values())))


if __name__ == "__main__":
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(errors="backslashreplace")
    sys.exit(main())
