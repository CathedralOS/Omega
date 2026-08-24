#!/usr/bin/env python3
"""Build and check the canonical two-package CKIR1 import fixture.

This is independent fixture/reference plumbing. It does not resolve imports or
claim compilation acceptance: the future Delta checker owns that relation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
FIXTURE = HERE / "fixtures" / "two-unit-import"
sys.path.insert(0, str(COMPILER))

import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402
import checked_ir_reference  # noqa: E402


DEP_KEY = "11" * 32
ROOT_KEY = "22" * 32
EXPECTED_SOURCE_BUNDLE_SHA256 = (
    "32145352472ac5566d808bf3796b41ced2c3ada186c63ff73c7ca1178ef5ea1f"
)
EXPECTED_ENVELOPE_SHA256 = (
    "bc1a986a0b10e3a7af3e0fb7ed44fcf8889bb67d1117b5d904b5a8d3236c49da"
)
EXPECTED_REFERENCE_BUNDLE_SHA256 = (
    "9905ab38a7127c7cea098f9fb438663a6db296fadd54d663aecf5eb545a90d99"
)
EXPECTED_REFERENCE_CKIR_SHA256 = (
    "0e8b6ea1b2e2300016f32c1087b9499a74c667042bd81083fc9388fc28141fcb"
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def expected_resolution() -> dict[str, object]:
    return {
        "schema": "omega-bootstrap-two-unit-reference-v1",
        "package_order": [DEP_KEY, ROOT_KEY],
        "source_order": ["dep/model.omg", "root/main.omg"],
        "module_order": ["model", "app"],
        "direct_aliases": [
            {"requester_package": 1, "alias": "dep", "target_package": 0}
        ],
        "import": {
            "requester_source": 1,
            "authored_path": "dep::model::Pair",
            "resolved_package": 0,
            "resolved_module": "model",
            "declaration": "Pair",
            "requires_public": True,
        },
        "declaration_order": [
            "package[0]::model::Pair",
            "package[1]::app::Probe",
            "package[1]::app::Probe::run",
        ],
        "selected_root": {
            "package": 1,
            "source": 1,
            "module": "app",
            "owner": "Probe",
            "machine": "run",
        },
        "ckir": {
            "schema": "CKIR1",
            "package_module_names_erased_after_resolution": True,
            "machine_calls": 0,
            "expected_scalar_result": 70,
            "expected_reference_input": "reference.bundle",
        },
    }


def load_manifest() -> dict[str, object]:
    raw = json.loads((FIXTURE / "manifest.json").read_text(encoding="utf-8"))
    require(raw["packages"][0]["key"] == DEP_KEY, "dependency key drift")
    require(raw["packages"][1]["key"] == ROOT_KEY, "root key drift")
    return raw


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    source_bundle = bundle.encode(
        [
            bundle.Entry("dep/model.omg", (FIXTURE / "dep-model.omg").read_bytes()),
            bundle.Entry("root/main.omg", (FIXTURE / "root-main.omg").read_bytes()),
        ]
    )
    envelope = compilation.encode_manifest(load_manifest(), source_bundle)
    decoded = compilation.decode(envelope)

    require([entry.label for entry in decoded.bundle_entries] == ["dep/model.omg", "root/main.omg"], "bundle order")
    require([package.key.hex() for package in decoded.packages] == [DEP_KEY, ROOT_KEY], "package order")
    require(decoded.root_package_id == 1 and decoded.root_source_id == 1, "selected root IDs")
    require(decoded.strings[decoded.root_owner_string_id] == "Probe", "selected owner")
    require(decoded.strings[decoded.root_machine_string_id] == "run", "selected machine")
    require(len(decoded.aliases) == 1, "direct alias count")
    alias = decoded.aliases[0]
    require(
        (alias.requester_package_id, decoded.strings[alias.alias_string_id], alias.target_package_id)
        == (1, "dep", 0),
        "direct alias relation",
    )

    reference_bundle = bundle.encode(
        [bundle.Entry("main.omg", (FIXTURE / "reference-flat.omg").read_bytes())]
    )
    source_digest = hashlib.sha256(source_bundle).hexdigest()
    envelope_digest = hashlib.sha256(envelope).hexdigest()
    reference_digest = hashlib.sha256(reference_bundle).hexdigest()
    require(source_digest == EXPECTED_SOURCE_BUNDLE_SHA256, "canonical source-bundle digest drift")
    require(envelope_digest == EXPECTED_ENVELOPE_SHA256, "canonical envelope digest drift")
    require(reference_digest == EXPECTED_REFERENCE_BUNDLE_SHA256, "reference-bundle digest drift")
    inspection = compilation.inspect(decoded)
    resolution = expected_resolution()

    (output / "source.bundle").write_bytes(source_bundle)
    (output / "compilation-envelope.bin").write_bytes(envelope)
    (output / "compilation-envelope.sha256").write_text(
        EXPECTED_ENVELOPE_SHA256 + "\n", encoding="ascii"
    )
    (output / "compilation-envelope.json").write_text(
        json.dumps(inspection, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (output / "reference.bundle").write_bytes(reference_bundle)
    (output / "expected-resolution.json").write_text(
        json.dumps(resolution, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    (output / "expected-observation.txt").write_text("70\n", encoding="ascii")


def check_module(contents: bytes) -> None:
    module = checked_ir_reference.decode(contents)
    require(module.entry != checked_ir_reference.NO_ID, "expected an entry-bearing CKIR module")
    require(len(module.tables["machines"]) == 1, "expected one call-free selected machine")
    require(len(module.tables["records"]) == 2, "expected Pair and Probe records")
    require(len(module.tables["fields"]) == 3, "expected Pair.first, Pair.second, and Probe.pair")
    pair, probe = module.tables["records"]
    require(pair[2:5] == (0, 2, 1), "Pair must be the first, copyable record")
    require(probe[2:5] == (2, 1, 0), "Probe must follow Pair and own one field")
    require(module.tables["fields"][2][3] == pair[1], "Probe field must use Pair's nominal type")
    machine = module.tables["machines"][0]
    require(machine[1] == 1 and machine[6:10] == (0, 0, 0, 1), "selected Probe machine shape")
    require(module.tables["types"][machine[5]][1] == 1, "selected result must be u8")
    require(checked_ir_reference.interpret(module) == 70, "CKIR result is not 70")


def check_ckir(path: Path) -> None:
    check_module(path.read_bytes())
    print("two-unit fixture CKIR1 valid: one machine, no calls, result 70")


def check_pair(expected: Path, actual: Path) -> None:
    expected_bytes = expected.read_bytes()
    actual_bytes = actual.read_bytes()
    require(
        hashlib.sha256(expected_bytes).hexdigest() == EXPECTED_REFERENCE_CKIR_SHA256,
        "resolved one-unit CKIR digest drift",
    )
    check_module(expected_bytes)
    check_module(actual_bytes)
    require(actual_bytes == expected_bytes, "two-unit CKIR differs from the resolved one-unit reference")
    print("two-unit fixture CKIR1 matches the canonical resolved reference and yields 70")


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    build_parser = subparsers.add_parser("build")
    build_parser.add_argument("output", type=Path)
    check_parser = subparsers.add_parser("check-ckir")
    check_parser.add_argument("ckir", type=Path)
    pair_parser = subparsers.add_parser("check-pair")
    pair_parser.add_argument("expected_ckir", type=Path)
    pair_parser.add_argument("actual_ckir", type=Path)
    arguments = parser.parse_args()
    if arguments.command == "build":
        build(arguments.output)
    elif arguments.command == "check-ckir":
        check_ckir(arguments.ckir)
    else:
        check_pair(arguments.expected_ckir, arguments.actual_ckir)


if __name__ == "__main__":
    try:
        main()
    except (
        ValueError,
        OSError,
        compilation.CompilationError,
        bundle.BundleError,
        checked_ir_reference.CkirError,
    ) as error:
        raise SystemExit(f"two-unit compilation fixture: {error}")
