#!/usr/bin/env python3
"""Matching-host runtime Value data-index acceptance; image-only runs never qualify."""

import argparse
import json
import os
from pathlib import Path
import sys

import compiler_pilot

PLACES = ("local", "field", "parameter", "call_result", "match_join", "const_name",
          "other_state", "other_machine", "target_body")
ROWS = ("runtime_index_application", "selected_value_application",
        "runtime_index_application_forwarded", "runtime_index_application_computed",
        "runtime_index_application_equivalent_subject")
GENERIC_PREFIXES = ("runtime_index_carrier_", "runtime_value_recipe_", "runtime_inferred_index_",
                    "runtime_index_borrow_call_", "selected_value_bool_public_contract_")
RUNTIME_NAMED = ("runtime_index_application_root_exit", "runtime_value_generic_subject_exit", "runtime_forwarded_value_contract_exit",
                 "static_callable_local_type_argument_exit", "runtime_computed_data_subject_exit",
                 "runtime_computed_initializer_capture_exit")
NAMED = RUNTIME_NAMED + ("const_generic_runtime_argument", "const_data_machine_call_runtime_argument")


def fixtures(root):
    """Pin every place/twin, then separate authored executable and source-only cases."""
    corpus = root / "tests/omega"
    selected = set()
    for tier in ("pass", "fail"):
        for prefix in ("runtime_index_application_", "selected_value_application_through_"):
            selected.update(path.relative_to(corpus).as_posix()
                            for path in (corpus / tier / "facts").glob(prefix + "*") if path.is_dir())
        for prefix in GENERIC_PREFIXES + NAMED:
            selected.update(path.relative_to(corpus).as_posix()
                            for path in (corpus / tier / "generics").glob(prefix + "*") if path.is_dir())
    for row in ROWS:
        for place in PLACES:
            for tier, suffix in (("pass", "exit"), ("fail", "rejected")):
                name = f"{tier}/facts/{row}_through_{place}_{suffix}"
                if name not in selected:
                    raise ValueError(f"required runtime Value place/twin is missing: {name}")
    for name in NAMED:
        if not any(Path(path).name == name for path in selected):
            raise ValueError(f"required runtime Value control is missing: {name}")
    runtime, checking_only, rejected = [], [], []
    for fixture in sorted(selected):
        path = corpus / fixture
        if not (path / "main.omg").is_file():
            raise ValueError(f"{fixture}: missing authored source")
        if fixture.startswith("fail/"):
            if not (path / "expected.txt").is_file():
                raise ValueError(f"{fixture}: missing intended diagnostic oracle")
            rejected.append(fixture)
            continue
        oracles = (path / "expected_native_exit.txt", path / "expected_interpreter_exit.txt")
        # Both-mode compares Terminal with the authored native oracle by
        # default; a separate interpreter oracle is an optional refinement.
        explicit = oracles[0].is_file()
        # The corpus also owns the established *_exit -> exit0 convention.
        # Fact rows additionally author Expected exit: 0 in source.
        authored_zero = (path.name.endswith("_exit")
                         and "// Expected exit: 0" in (path / "main.omg").read_text(encoding="utf-8"))
        if (explicit or authored_zero) and (path / "build.omg").is_file():
            runtime.append(fixture)
        elif fixture.startswith("pass/facts/"):
            raise ValueError(f"{fixture}: fact place lacks native/Terminal or entry oracle")
        else:
            checking_only.append(fixture)
    for name in RUNTIME_NAMED:
        if f"pass/generics/{name}" not in runtime:
            raise ValueError(f"required runtime witness lacks execution oracles or entry: {name}")
    return tuple(runtime), tuple(rejected), tuple(checking_only)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true", help="show the exact cohort without building")
    args = parser.parse_args()
    root = compiler_pilot.ROOT
    try:
        positives, negatives, checking_only = fixtures(root)
    except (OSError, ValueError) as error:
        print(f"runtime Value cohort refused: {error}", file=sys.stderr)
        return 1
    if args.list:
        print(json.dumps(dict(positive=positives, negative=negatives,
                              checking_only=checking_only), indent=2))
        return 0
    os.environ["OMEGA_LIBRARY_ROOT"] = str(root / "source/library")
    return compiler_pilot.run_suite("compiler-runtime-value-generics", positives, negatives,
                                    checking_only=checking_only)


if __name__ == "__main__":
    sys.exit(main())
