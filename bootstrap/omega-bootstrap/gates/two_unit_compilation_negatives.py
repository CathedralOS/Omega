#!/usr/bin/env python3
"""Build and check semantic-negative inputs for the two-unit import fixture.

Every generated envelope is structurally valid. The cases are inputs and
expectations for source resolution; this tool does not claim compilation
acceptance or perform the expected semantic rejection.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path


HERE = Path(__file__).resolve().parent
COMPILER = HERE.parent / "compiler"
FIXTURE = HERE / "fixtures" / "two-unit-import"
sys.path.insert(0, str(COMPILER))

import omega_bootstrap_bundle as bundle  # noqa: E402
import omega_bootstrap_compilation as compilation  # noqa: E402


DEP_KEY = "11" * 32
MIDDLE_KEY = "18" * 32
ROOT_KEY = "22" * 32
EXPECTED_DIGESTS = {
    "missing-direct-alias": (
        "32145352472ac5566d808bf3796b41ced2c3ada186c63ff73c7ca1178ef5ea1f",
        "2598f35d0776506329f0e269045dad4142036803052b449b3830e079c4631528",
    ),
    "transitive-only-reach": (
        "6ac9428fd3af40ebd4241a48c66d814f06ce91de2db6fa4c28f645f3880004ae",
        "6864dd06c8bfcedfcd245dd84e41a95789984f5b0ede2f0fe62ef1343a2dd5bb",
    ),
    "private-import": (
        "1d5211a295c12208dfa375bff89ba00e471f9acb1bd2cbdc5822dd4002c20c22",
        "d2f2e0285d61c4bde1156495f702c2f6531190bd8217680aafac1ead3ce55028",
    ),
    "module-mismatch-dependency": (
        "e3ad65d2e804656abf6a3f9123b40bae1daa8db4ddebc3aad8994f927c0c66af",
        "f62a53b6b63062f3f22aee4eb0114a1ebda977265eca89d7542369f0f2e6a048",
    ),
    "module-mismatch-root": (
        "7e183eb2bfb6dc6337200f8f8c5d3e0a2e14caa44cbea29c4ae711807367f623",
        "10ae02135b18894a735fd45f0e628ff220a66d591c3597247339236e8434080f",
    ),
    "alias-module-ambiguity": (
        "6f5032f37e291899eccc633d4ffc9727e491d5971e7e73da4cb5dbcb94ba4585",
        "f9a28cfbdfa438cd6afaec3a5d9dc90954255b3e9738283b9dd84a7546d44adb",
    ),
    "duplicate-identity": (
        "4e91089db24e4c7860aa1164f9e85800c4af3125c5135533e9fdbf3be6a27d29",
        "7fe845a382d45ed120ec6937a6ee7c1eb7ca66b01e0ef20d40bfb5dfcc2a4b54",
    ),
    "wrong-selected-root-source": (
        "5f3c1f030bdbeacff1e846a61484ebcb97e2ca59bfb529308b668d7300154ebf",
        "c506963a9becdaf7350291a722d5578b895170360e61e98ccad282ac3adb4755",
    ),
    "wrong-selected-root-owner": (
        "32145352472ac5566d808bf3796b41ced2c3ada186c63ff73c7ca1178ef5ea1f",
        "033fe6886a2d387fff8866e447c01d792d5e4324e61f261fd1b379091642e87f",
    ),
    "wrong-selected-root-machine": (
        "32145352472ac5566d808bf3796b41ced2c3ada186c63ff73c7ca1178ef5ea1f",
        "3d780db966017d5c21a672e091cac89ef4e1d2e45a7a8ad4a02d454491789cd3",
    ),
}


@dataclass(frozen=True)
class NegativeCase:
    name: str
    sources: tuple[bundle.Entry, ...]
    manifest: dict[str, object]
    reason: str


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def source(text: str) -> bytes:
    return text.strip().encode("ascii") + b"\n"


MIDDLE_SOURCE = source(
    """
module bridge;

pub data Marker [copy] {
    value: u8;
}
"""
)
LOCAL_DEP_MODULE_SOURCE = source(
    """
module dep;

data LocalMarker [copy] {
    value: u8;
}
"""
)
DUPLICATE_PAIR_SOURCE = source(
    """
module model;

pub data Pair [copy] {
    first: u8;
    second: u8;
}
"""
)
WRONG_ROOT_SOURCE = source(
    """
module app;

data Helper [copy] {
    value: u8;
}
"""
)


def package(key: str, *sources: tuple[str, str]) -> dict[str, object]:
    return {
        "key": key,
        "sources": [
            {"label": label, "module": module} for label, module in sources
        ],
    }


def alias(requester: str, name: str, target: str) -> dict[str, str]:
    return {"requester": requester, "alias": name, "target": target}


def manifest(
    packages: list[dict[str, object]],
    aliases: list[dict[str, str]],
    *,
    root_source: str = "root/main.omg",
    root_owner: str = "Probe",
    root_machine: str = "run",
) -> dict[str, object]:
    return {
        "target": "linux_x86_64",
        "packages": packages,
        "aliases": aliases,
        "root": {
            "package": ROOT_KEY,
            "source": root_source,
            "owner": root_owner,
            "machine": root_machine,
        },
    }


def negative_cases() -> tuple[NegativeCase, ...]:
    dep = (FIXTURE / "dep-model.omg").read_bytes()
    root = (FIXTURE / "root-main.omg").read_bytes()
    private_dep = dep.replace(b"pub data Pair", b"data Pair", 1)
    mismatched_dep = dep.replace(b"module model;", b"module wrong_model;", 1)
    mismatched_root = root.replace(b"module app;", b"module wrong_app;", 1)
    ordinary_packages = [
        package(DEP_KEY, ("dep/model.omg", "model")),
        package(ROOT_KEY, ("root/main.omg", "app")),
    ]
    ordinary_sources = (
        bundle.Entry("dep/model.omg", dep),
        bundle.Entry("root/main.omg", root),
    )
    direct_dep = [alias(ROOT_KEY, "dep", DEP_KEY)]
    return (
        NegativeCase(
            "missing-direct-alias",
            ordinary_sources,
            manifest(ordinary_packages, [alias(ROOT_KEY, "dependency", DEP_KEY)]),
            "the requester has no direct alias named dep; differently named reach is insufficient",
        ),
        NegativeCase(
            "transitive-only-reach",
            (
                bundle.Entry("dep/model.omg", dep),
                bundle.Entry("middle/bridge.omg", MIDDLE_SOURCE),
                bundle.Entry("root/main.omg", root),
            ),
            manifest(
                [
                    package(DEP_KEY, ("dep/model.omg", "model")),
                    package(MIDDLE_KEY, ("middle/bridge.omg", "bridge")),
                    package(ROOT_KEY, ("root/main.omg", "app")),
                ],
                [
                    alias(MIDDLE_KEY, "dep", DEP_KEY),
                    alias(ROOT_KEY, "middle", MIDDLE_KEY),
                ],
            ),
            "dep is an alias only for the middle requester, not for root",
        ),
        NegativeCase(
            "private-import",
            (
                bundle.Entry("dep/model.omg", private_dep),
                bundle.Entry("root/main.omg", root),
            ),
            manifest(ordinary_packages, direct_dep),
            "cross-package declaration Pair is not public",
        ),
        NegativeCase(
            "module-mismatch-dependency",
            (
                bundle.Entry("dep/model.omg", mismatched_dep),
                bundle.Entry("root/main.omg", root),
            ),
            manifest(ordinary_packages, direct_dep),
            "dependency source authors wrong_model but its envelope module is model",
        ),
        NegativeCase(
            "module-mismatch-root",
            (
                bundle.Entry("dep/model.omg", dep),
                bundle.Entry("root/main.omg", mismatched_root),
            ),
            manifest(ordinary_packages, direct_dep),
            "selected source authors wrong_app but its envelope module is app",
        ),
        NegativeCase(
            "alias-module-ambiguity",
            (
                bundle.Entry("dep/model.omg", dep),
                bundle.Entry("root/dep.omg", LOCAL_DEP_MODULE_SOURCE),
                bundle.Entry("root/main.omg", root),
            ),
            manifest(
                [
                    package(DEP_KEY, ("dep/model.omg", "model")),
                    package(
                        ROOT_KEY,
                        ("root/dep.omg", "dep"),
                        ("root/main.omg", "app"),
                    ),
                ],
                direct_dep,
            ),
            "dep denotes both a requester-local package alias and a same-package top-level module",
        ),
        NegativeCase(
            "duplicate-identity",
            (
                bundle.Entry("dep/model.omg", dep),
                bundle.Entry("dep/pair-again.omg", DUPLICATE_PAIR_SOURCE),
                bundle.Entry("root/main.omg", root),
            ),
            manifest(
                [
                    package(
                        DEP_KEY,
                        ("dep/model.omg", "model"),
                        ("dep/pair-again.omg", "model"),
                    ),
                    package(ROOT_KEY, ("root/main.omg", "app")),
                ],
                direct_dep,
            ),
            "Pair has two declarations in the same package and logical module",
        ),
        NegativeCase(
            "wrong-selected-root-source",
            ordinary_sources
            + (bundle.Entry("root/helper.omg", WRONG_ROOT_SOURCE),),
            manifest(
                [
                    package(DEP_KEY, ("dep/model.omg", "model")),
                    package(
                        ROOT_KEY,
                        ("root/helper.omg", "app"),
                        ("root/main.omg", "app"),
                    ),
                ],
                direct_dep,
                root_source="root/helper.omg",
            ),
            "Probe::run exists only outside the selected source",
        ),
        NegativeCase(
            "wrong-selected-root-owner",
            ordinary_sources,
            manifest(ordinary_packages, direct_dep, root_owner="MissingProbe"),
            "selected owner MissingProbe is not authored in the selected source and module",
        ),
        NegativeCase(
            "wrong-selected-root-machine",
            ordinary_sources,
            manifest(ordinary_packages, direct_dep, root_machine="missing_run"),
            "selected machine Probe::missing_run is not authored in the selected source and module",
        ),
    )


def build(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    cases = negative_cases()
    require(
        set(EXPECTED_DIGESTS) == {case.name for case in cases},
        "negative digest inventory drift",
    )
    index_cases: list[dict[str, object]] = []
    for case in cases:
        case_output = output / case.name
        case_output.mkdir(parents=True, exist_ok=True)
        source_bundle = bundle.encode(list(case.sources))
        envelope = compilation.encode_manifest(case.manifest, source_bundle)
        decoded = compilation.decode(envelope)
        source_digest = hashlib.sha256(source_bundle).hexdigest()
        envelope_digest = hashlib.sha256(envelope).hexdigest()
        require(
            (source_digest, envelope_digest) == EXPECTED_DIGESTS[case.name],
            f"{case.name}: pinned negative digest drift",
        )
        expectation = {
            "schema": "omega-bootstrap-two-unit-negative-v1",
            "case": case.name,
            "classification": "source-resolution-rejection",
            "envelope_structurally_valid": True,
            "expected_status": 251,
            "must_reject_before": "CKIR-or-artifact-publication",
            "reason": case.reason,
            "source_bundle_sha256": source_digest,
            "compilation_envelope_sha256": envelope_digest,
        }
        for entry in case.sources:
            source_path = case_output / "sources" / entry.label
            source_path.parent.mkdir(parents=True, exist_ok=True)
            source_path.write_bytes(entry.content)
        (case_output / "manifest.json").write_text(
            json.dumps(case.manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (case_output / "source.bundle").write_bytes(source_bundle)
        (case_output / "compilation-envelope.bin").write_bytes(envelope)
        (case_output / "compilation-envelope.json").write_text(
            json.dumps(compilation.inspect(decoded), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (case_output / "expected-rejection.json").write_text(
            json.dumps(expectation, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        index_cases.append(
            {
                "case": case.name,
                "compilation_envelope_sha256": envelope_digest,
                "source_bundle_sha256": source_digest,
            }
        )
    (output / "index.json").write_text(
        json.dumps(
            {
                "schema": "omega-bootstrap-two-unit-negative-index-v1",
                "cases": index_cases,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def check(output: Path) -> None:
    cases = negative_cases()
    expected_names = [case.name for case in cases]
    index = json.loads((output / "index.json").read_text(encoding="utf-8"))
    require(
        index.get("schema") == "omega-bootstrap-two-unit-negative-index-v1",
        "negative index schema drift",
    )
    rows = index.get("cases")
    require(isinstance(rows, list), "negative index cases must be an array")
    require(
        [row.get("case") for row in rows if isinstance(row, dict)] == expected_names,
        "negative case inventory or order drift",
    )
    for case, row in zip(cases, rows):
        require(isinstance(row, dict), f"{case.name}: index row must be an object")
        case_output = output / case.name
        source_bundle = (case_output / "source.bundle").read_bytes()
        envelope = (case_output / "compilation-envelope.bin").read_bytes()
        emitted_manifest = json.loads(
            (case_output / "manifest.json").read_text(encoding="utf-8")
        )
        require(emitted_manifest == case.manifest, f"{case.name}: manifest drift")
        require(
            source_bundle == bundle.encode(list(case.sources)),
            f"{case.name}: source bundle differs from its generated sources",
        )
        require(
            envelope == compilation.encode_manifest(case.manifest, source_bundle),
            f"{case.name}: compilation envelope differs from its manifest and source bundle",
        )
        decoded = compilation.decode(envelope)
        require(
            decoded.bundle_entries == tuple(bundle.decode(source_bundle)),
            f"{case.name}: nested bundle drift",
        )
        emitted_inspection = json.loads(
            (case_output / "compilation-envelope.json").read_text(encoding="utf-8")
        )
        require(
            emitted_inspection == compilation.inspect(decoded),
            f"{case.name}: envelope inspection drift",
        )
        envelope_digest = hashlib.sha256(envelope).hexdigest()
        source_digest = hashlib.sha256(source_bundle).hexdigest()
        require(
            (source_digest, envelope_digest) == EXPECTED_DIGESTS[case.name],
            f"{case.name}: pinned negative digest drift",
        )
        require(
            row.get("compilation_envelope_sha256") == envelope_digest
            and row.get("source_bundle_sha256") == source_digest,
            f"{case.name}: negative index digest drift",
        )
        expectation = json.loads(
            (case_output / "expected-rejection.json").read_text(encoding="utf-8")
        )
        require(
            expectation.get("case") == case.name
            and expectation.get("classification") == "source-resolution-rejection"
            and expectation.get("envelope_structurally_valid") is True
            and expectation.get("expected_status") == 251
            and expectation.get("must_reject_before") == "CKIR-or-artifact-publication"
            and expectation.get("reason") == case.reason,
            f"{case.name}: rejection expectation drift",
        )
        require(
            expectation.get("compilation_envelope_sha256") == envelope_digest
            and expectation.get("source_bundle_sha256") == source_digest,
            f"{case.name}: rejection digest drift",
        )
        for entry in case.sources:
            require(
                (case_output / "sources" / entry.label).read_bytes() == entry.content,
                f"{case.name}: generated source drift for {entry.label}",
            )
    print(
        f"two-unit negative fixtures valid: {len(cases)} structural envelopes, "
        "semantic rejection expected"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)
    build_parser = subparsers.add_parser("build")
    build_parser.add_argument("output", type=Path)
    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("output", type=Path)
    arguments = parser.parse_args()
    if arguments.command == "build":
        build(arguments.output)
    else:
        check(arguments.output)


if __name__ == "__main__":
    try:
        main()
    except (
        ValueError,
        OSError,
        json.JSONDecodeError,
        compilation.CompilationError,
        bundle.BundleError,
    ) as error:
        raise SystemExit(f"two-unit compilation negatives: {error}")
