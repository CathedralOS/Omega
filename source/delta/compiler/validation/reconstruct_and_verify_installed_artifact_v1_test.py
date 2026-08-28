#!/usr/bin/env python3
"""Focused tests for reconstruction from the retained Delta installation."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import lower_rooted_assembly_publication_v1 as publication
from lower_rooted_assembly_publication_v1_test import gamma_observation
from publication_support_test import POSITIVE_ASSEMBLY


HERE = Path(__file__).resolve().parent
RUNNER_PATH = HERE / "reconstruct-and-verify-installed-artifact-v1.py"
SPEC = importlib.util.spec_from_file_location("delta_installed_reconstruction", RUNNER_PATH)
assert SPEC is not None and SPEC.loader is not None
runner = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = runner
SPEC.loader.exec_module(runner)


class InstalledReconstructionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.installation = self.root / "darwin-arm64-v1"
        self.installation.mkdir()
        self.raw = self.installation / "execution.raw"
        self.raw.write_bytes(gamma_observation(POSITIVE_ASSEMBLY))
        for name in (
            "artifact-custody-receipt.json",
            "delta-compiler",
            "installation.json",
            "realization-observation.json",
        ):
            (self.installation / name).write_bytes(b"fixture\n")
        self.tools = [self.root / name for name in ("clang", "ld", "SDKSettings.json", "libSystem.tbd", "runtime.a")]
        for path in self.tools:
            path.write_bytes(b"tool\n")
        self.gate = self.root / "fake-verify-installed"
        self.gate_log = self.root / "gate-log.json"
        self.write_receipt()

    def write_receipt(self) -> None:
        shared = {
            "assembly": {"sha256": "a" * 64},
            "gamma_stdout": {"sha256": "b" * 64},
            "stderr": {"sha256": "c" * 64},
        }
        receipt = {
            "elaboration": {"elapsed_milliseconds": 101, "status": 0},
            "executions": [
                {**shared, "elapsed_milliseconds": 201, "ordinal": 0, "semantic_status": 0, "status": 0},
                {**shared, "elapsed_milliseconds": 202, "ordinal": 1, "semantic_status": 0, "status": 0},
            ],
            "publication_id": publication.PUBLICATION_ID,
            "receipt_sha256": "0" * 64,
            "schema": publication.RECEIPT_SCHEMA,
        }
        receipt["receipt_sha256"] = publication.receipt_digest(receipt)
        (self.installation / "assembly-publication-receipt.json").write_bytes(
            publication.canonical_json(receipt, pretty=True)
        )

    def write_gate(self, status: int = 0, stderr: str = "") -> None:
        program = f"""#!/usr/bin/env python3
import json
import sys
from pathlib import Path
arguments = sys.argv[1:]
record = {{
    'arguments': arguments,
    'elaboration': json.loads(Path(arguments[13]).read_text()),
    'execution0': json.loads(Path(arguments[15]).read_text()),
    'execution1': json.loads(Path(arguments[18]).read_text()),
    'temporary_root': str(Path(arguments[8]).parent.parent),
}}
Path({str(self.gate_log)!r}).write_text(json.dumps(record))
sys.stderr.write({stderr!r})
raise SystemExit({status})
"""
        self.gate.write_text(program)
        self.gate.chmod(0o755)

    def fake_build_tools(self, root: Path) -> None:
        artifacts = root / "artifacts"
        artifacts.mkdir()
        for name in ("assembler.tape", "delta2gamma.tape", "interp.tape"):
            (artifacts / name).write_bytes(f"fixture {name}\n".encode())
        (artifacts / "delta2gamma.exe").write_bytes(b"fixture translator\n")

    def run_reconstruction(self) -> None:
        with (
            mock.patch.object(runner, "VERIFY_INSTALLED", self.gate),
            mock.patch.object(runner.driver, "build_tools", side_effect=self.fake_build_tools),
            mock.patch.object(
                runner.driver,
                "run_short",
                side_effect=lambda argv, stdin, context: self.assert_translator_only(argv),
            ),
            mock.patch.object(
                runner.support,
                "materialize_canonical_image",
                return_value=b"source image\n",
            ),
        ):
            runner.reconstruct_and_verify(self.installation, *self.tools)

    def assert_translator_only(self, argv: list[str]) -> bytes:
        self.assertEqual(Path(argv[0]).name, "delta2gamma.exe")
        return b"(let compiler_input STDIN 0)\n"

    def test_reconstructs_minimal_evidence_without_gamma_execution(self) -> None:
        self.write_gate()
        self.run_reconstruction()
        record = json.loads(self.gate_log.read_text())
        arguments = record["arguments"]
        self.assertEqual(len(arguments), 22)
        self.assertEqual(arguments[19], str(self.raw))
        self.assertEqual(arguments[16], arguments[20])
        self.assertEqual(
            {arguments[index] for index in (1, 2, 14, 17, 21)},
            {arguments[1]},
        )
        self.assertEqual(record["elaboration"]["elapsed_milliseconds"], 101)
        self.assertEqual(record["execution0"]["elapsed_milliseconds"], 201)
        self.assertEqual(record["execution0"]["ordinal"], 0)
        self.assertEqual(record["execution1"]["elapsed_milliseconds"], 202)
        self.assertEqual(record["execution1"]["ordinal"], 1)
        self.assertFalse(Path(record["temporary_root"]).exists())

    def test_malformed_receipt_rejects_before_reconstruction(self) -> None:
        (self.installation / "assembly-publication-receipt.json").write_bytes(b"{}\n")
        with mock.patch.object(runner.driver, "build_tools") as build:
            with self.assertRaises((runner.ReconstructionError, publication.ReceiptError)):
                runner.reconstruct_and_verify(self.installation, *self.tools)
        build.assert_not_called()

    def test_custody_rejection_cleans_all_temporary_evidence(self) -> None:
        self.write_gate(251, "custody rejected\n")
        with self.assertRaisesRegex(runner.ReconstructionError, "custody rejected"):
            self.run_reconstruction()
        record = json.loads(self.gate_log.read_text())
        self.assertFalse(Path(record["temporary_root"]).exists())

    def test_relative_public_input_rejects(self) -> None:
        with self.assertRaises(runner.ReconstructionUsageError):
            runner.reconstruct_and_verify(Path("relative"), *self.tools)


if __name__ == "__main__":
    unittest.main()
