#!/usr/bin/env python3
"""Focused no-publication tests for the Delta assembly evidence join."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import lower_rooted_assembly_publication_v1 as publication
from publication_support_test import POSITIVE_ASSEMBLY


HERE = Path(__file__).resolve().parent
MANIFEST = HERE / "source-closures/canonical-compiler-v1.json"
LOCATIONS = HERE / "source-closures/canonical-compiler-v1.locations.json"


def gamma_observation(assembly: bytes) -> bytes:
    cells = "".join(f"(Cons {byte} " for byte in assembly)
    return f"(Pair 0 {cells}Nil{')' * len(assembly)})\n".encode("ascii")


class Evidence:
    def __init__(self, root: Path) -> None:
        self.assembler = root / "assembler.tape"
        self.translator = root / "omega2gamma.tape"
        self.interpreter = root / "interp.tape"
        self.template = root / "template.gamma"
        self.gamma = root / "closed.gamma"
        self.elaboration_stderr = root / "elaboration.err"
        self.elaboration = root / "elaboration.json"
        self.execution = (root / "execution-0.json", root / "execution-1.json")
        self.raw = (root / "execution-0.raw", root / "execution-1.raw")
        self.assembly = (root / "execution-0.s", root / "execution-1.s")
        self.stderr = (root / "execution-0.err", root / "execution-1.err")
        self.receipt = root / "receipt.json"

        self.assembler.write_bytes(b"test-only deterministic assembler tape\n")
        self.translator.write_bytes(b"test-only deterministic translator tape\n")
        self.interpreter.write_bytes(b"test-only deterministic interpreter tape\n")
        self.template.write_bytes(b"(let compiler_input STDIN 0)\n")
        image = publication.support.materialize_canonical_image(
            MANIFEST, LOCATIONS, {"delta": HERE}
        )
        packer = publication._load_module("publication_test_packer", publication.PACKER_SOURCE)
        self.gamma.write_bytes(packer.inject(self.template.read_bytes(), image))
        self.elaboration_stderr.write_bytes(b"")
        for ordinal in (0, 1):
            self.assembly[ordinal].write_bytes(POSITIVE_ASSEMBLY)
            self.raw[ordinal].write_bytes(gamma_observation(POSITIVE_ASSEMBLY))
            self.stderr[ordinal].write_bytes(b"")
        self.write_observations()

    @property
    def roots(self) -> dict[str, Path]:
        return {"delta": HERE}

    def write_observations(self) -> None:
        elaboration = publication.make_elaboration_observation(
            0, 101, MANIFEST, LOCATIONS, self.roots, self.assembler,
            self.translator, self.interpreter, self.template,
            self.elaboration_stderr,
        )
        self.elaboration.write_bytes(publication.canonical_json(elaboration, pretty=True))
        for ordinal in (0, 1):
            execution = publication.make_execution_observation(
                ordinal, 0, 200 + ordinal, MANIFEST, LOCATIONS, self.roots,
                self.assembler, self.translator, self.interpreter, self.template,
                self.gamma, self.raw[ordinal], self.assembly[ordinal],
                self.stderr[ordinal],
            )
            self.execution[ordinal].write_bytes(
                publication.canonical_json(execution, pretty=True)
            )

    def join_arguments(self) -> list[str]:
        return [
            str(MANIFEST), str(LOCATIONS), str(self.assembler),
            str(self.translator), str(self.interpreter), str(self.template),
            str(self.gamma), str(self.elaboration), str(self.elaboration_stderr),
            str(self.execution[0]), str(self.raw[0]), str(self.assembly[0]),
            str(self.stderr[0]), str(self.execution[1]), str(self.raw[1]),
            str(self.assembly[1]), str(self.stderr[1]), f"delta={HERE}",
        ]


class PublicationJoinTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.evidence = Evidence(Path(self.temporary.name))

    def run_cli(self, *arguments: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, str(HERE / "lower_rooted_assembly_publication_v1.py"), *arguments],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )

    def assert_rejects(self, status: int, *arguments: str) -> None:
        result = self.run_cli(*arguments)
        self.assertEqual(result.returncode, status, result.stderr)
        self.assertEqual(result.stdout, b"")

    def test_exact_generate_and_verify(self) -> None:
        generated = self.run_cli("generate", *self.evidence.join_arguments())
        self.assertEqual(generated.returncode, 0, generated.stderr)
        receipt = json.loads(generated.stdout)
        self.assertEqual(receipt["claim"], publication.CLAIM)
        self.assertEqual(receipt["toolchain"]["alpha_vm"]["host"], "macos_arm64")
        self.assertEqual(
            receipt["toolchain"]["alpha_vm"]["authority_role"],
            "audited_alpha_macos_arm64_host_seed",
        )
        self.assertEqual(receipt["receipt_sha256"], publication.receipt_digest(receipt))
        self.assertNotEqual(
            receipt["executions"][0]["elapsed_milliseconds"],
            receipt["executions"][1]["elapsed_milliseconds"],
        )
        self.evidence.receipt.write_bytes(generated.stdout)
        verified = self.run_cli(
            "verify", str(self.evidence.receipt), *self.evidence.join_arguments()
        )
        self.assertEqual(verified.returncode, 0, verified.stderr)
        self.assertEqual(verified.stdout, b"")
        shell_arguments = [
            str(self.evidence.receipt), str(self.evidence.assembler),
            str(self.evidence.translator), str(self.evidence.interpreter),
            str(self.evidence.template), str(self.evidence.gamma),
            str(self.evidence.elaboration), str(self.evidence.elaboration_stderr),
            str(self.evidence.execution[0]), str(self.evidence.raw[0]),
            str(self.evidence.assembly[0]), str(self.evidence.stderr[0]),
            str(self.evidence.execution[1]), str(self.evidence.raw[1]),
            str(self.evidence.assembly[1]), str(self.evidence.stderr[1]),
        ]
        shell = subprocess.run(
            [str(HERE / "lower-rooted-assembly-publication-v1.sh"), *shell_arguments],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
        self.assertEqual(shell.returncode, 0, shell.stderr)
        self.assertIn(b"PASS", shell.stdout)

    def test_observation_commands_recompute_relations(self) -> None:
        elaboration = self.run_cli(
            "observe-elaboration", "0", "101", str(MANIFEST), str(LOCATIONS),
            str(self.evidence.assembler), str(self.evidence.translator),
            str(self.evidence.interpreter), str(self.evidence.template),
            str(self.evidence.elaboration_stderr), f"delta={HERE}",
        )
        self.assertEqual(elaboration.returncode, 0, elaboration.stderr)
        self.assertEqual(elaboration.stdout, self.evidence.elaboration.read_bytes())
        execution = self.run_cli(
            "observe-execution", "0", "0", "200", str(MANIFEST), str(LOCATIONS),
            str(self.evidence.assembler), str(self.evidence.translator),
            str(self.evidence.interpreter), str(self.evidence.template),
            str(self.evidence.gamma), str(self.evidence.raw[0]),
            str(self.evidence.assembly[0]), str(self.evidence.stderr[0]),
            f"delta={HERE}",
        )
        self.assertEqual(execution.returncode, 0, execution.stderr)
        self.assertEqual(execution.stdout, self.evidence.execution[0].read_bytes())

    def test_nonzero_or_diagnostic_execution_rejects(self) -> None:
        args = [
            "observe-execution", "0", "1", "200", str(MANIFEST), str(LOCATIONS),
            str(self.evidence.assembler), str(self.evidence.translator),
            str(self.evidence.interpreter), str(self.evidence.template),
            str(self.evidence.gamma), str(self.evidence.raw[0]),
            str(self.evidence.assembly[0]), str(self.evidence.stderr[0]),
            f"delta={HERE}",
        ]
        self.assert_rejects(251, *args)
        self.evidence.stderr[0].write_bytes(b"diagnostic\n")
        args[3] = "0"
        self.assert_rejects(251, *args)

    def test_template_and_closed_gamma_cross_pairs_reject(self) -> None:
        self.evidence.template.write_bytes(b"(let changed STDIN 0)\n")
        self.assert_rejects(251, "generate", *self.evidence.join_arguments())
        self.evidence.template.write_bytes(b"(let compiler_input STDIN 0)\n")
        self.evidence.gamma.write_bytes(self.evidence.gamma.read_bytes() + b" ")
        self.assert_rejects(251, "generate", *self.evidence.join_arguments())

    def test_tool_source_and_artifact_cross_pairs_reject(self) -> None:
        self.evidence.interpreter.write_bytes(b"changed interpreter tape\n")
        self.assert_rejects(251, "generate", *self.evidence.join_arguments())
        self.evidence.interpreter.write_bytes(b"test-only deterministic interpreter tape\n")
        candidate = json.loads(self.evidence.execution[0].read_bytes())
        candidate["toolchain"]["translator"]["source"]["sha256"] = "0" * 64
        self.evidence.execution[0].write_bytes(publication.canonical_json(candidate, pretty=True))
        self.assert_rejects(251, "generate", *self.evidence.join_arguments())

    def test_raw_decode_assembly_and_repeatability_teeth(self) -> None:
        self.evidence.raw[0].write_bytes(gamma_observation(POSITIVE_ASSEMBLY) + b"trailing")
        self.assert_rejects(251, "generate", *self.evidence.join_arguments())

        self.evidence.raw[0].write_bytes(gamma_observation(POSITIVE_ASSEMBLY))
        changed = POSITIVE_ASSEMBLY.replace(b"    .byte 65,66,67\n", b"    .byte 65,66,68\n")
        self.evidence.assembly[1].write_bytes(changed)
        self.evidence.raw[1].write_bytes(gamma_observation(changed))
        execution = publication.make_execution_observation(
            1, 0, 201, MANIFEST, LOCATIONS, self.evidence.roots,
            self.evidence.assembler, self.evidence.translator,
            self.evidence.interpreter, self.evidence.template, self.evidence.gamma,
            self.evidence.raw[1], self.evidence.assembly[1], self.evidence.stderr[1],
        )
        self.evidence.execution[1].write_bytes(publication.canonical_json(execution, pretty=True))
        self.assert_rejects(251, "generate", *self.evidence.join_arguments())

    def test_invalid_assembly_and_receipt_mutation_reject(self) -> None:
        bad = POSITIVE_ASSEMBLY.replace(b".global _main", b".global _other")
        self.evidence.assembly[0].write_bytes(bad)
        self.evidence.raw[0].write_bytes(gamma_observation(bad))
        self.assert_rejects(251, "generate", *self.evidence.join_arguments())

        self.evidence.assembly[0].write_bytes(POSITIVE_ASSEMBLY)
        self.evidence.raw[0].write_bytes(gamma_observation(POSITIVE_ASSEMBLY))
        receipt = publication.make_receipt(*publication.parse_join(self.evidence.join_arguments()))
        receipt["target"]["configuration"] = "optimized"
        self.evidence.receipt.write_bytes(publication.canonical_json(receipt, pretty=True))
        self.assert_rejects(
            251, "verify", str(self.evidence.receipt), *self.evidence.join_arguments()
        )

    def test_resource_and_missing_evidence_teeth(self) -> None:
        self.evidence.interpreter.write_bytes(b"x" * (publication.MAX_TAPE + 1))
        self.assert_rejects(252, "generate", *self.evidence.join_arguments())
        self.evidence.execution[0].write_bytes(b" " * (publication.MAX_DOCUMENT + 1))
        self.assert_rejects(252, "generate", *self.evidence.join_arguments())
        missing = subprocess.run(
            [str(HERE / "lower-rooted-assembly-publication-v1.sh")],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
        self.assertEqual(missing.returncode, 2)
        self.assertEqual(missing.stdout, b"")


if __name__ == "__main__":
    unittest.main()
