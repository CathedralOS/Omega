#!/usr/bin/env python3
"""Focused, no-long-run tests for the Delta publication attempt driver."""

from __future__ import annotations

import json
import platform
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

import lower_rooted_assembly_publication_v1 as publication
import lower_rooted_assembly_publication_v1_driver as driver
from publication_support_test import POSITIVE_ASSEMBLY


HERE = Path(__file__).resolve().parent


def gamma_observation(assembly: bytes) -> bytes:
    cells = "".join(f"(Cons {byte} " for byte in assembly)
    return f"(Pair 0 {cells}Nil{')' * len(assembly)})\n".encode("ascii")


def write_marker(root: Path, stage: str, *, token: str | None = None,
                 start_delta: int = 1, status: int = 0) -> None:
    plan = driver.load_plan(root)
    inputs, outputs = driver.stage_paths(root, stage)
    start = plan["prepared_epoch_ns"] + start_delta
    attempt = token if token is not None else plan["attempt_id"]
    started = {
        "attempt_id": attempt,
        "inputs": driver.marker_identity(inputs, f"{stage}_input"),
        "prepared_epoch_ns": plan["prepared_epoch_ns"],
        "schema": driver.MARKER_SCHEMA,
        "stage": stage,
        "start_epoch_ns": start,
    }
    finished = {
        **started,
        "elapsed_milliseconds": 10 + len(stage),
        "finish_epoch_ns": start + 1,
        "outputs": driver.marker_identity(outputs, f"{stage}_output"),
        "status": status,
    }
    (root / f"{stage}.started.json").write_bytes(driver.canonical_json(started))
    (root / f"{stage}.finished.json").write_bytes(driver.canonical_json(finished))


class DriverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        if (platform.system(), platform.machine()) != ("Darwin", "arm64"):
            raise unittest.SkipTest("exact prepare profile is Darwin arm64")
        cls.base_temporary = tempfile.TemporaryDirectory()
        cls.base = Path(cls.base_temporary.name) / "attempt"
        driver.prepare(cls.base)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.base_temporary.cleanup()

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "attempt"
        shutil.copytree(self.base, self.root)

    def run_cli(self, command: str) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, "-B", str(HERE / "lower_rooted_assembly_publication_v1_driver.py"),
             command, str(self.root)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def materialize_completed_outputs(self) -> None:
        template = b"(let compiler_input STDIN 0)\n"
        (self.root / "template.gamma").write_bytes(template)
        (self.root / "elaboration.stderr").write_bytes(b"")
        image = (self.root / "canonical-source.lf").read_bytes()
        packer = publication._load_module("driver_test_packer", publication.PACKER_SOURCE)
        (self.root / "closed.gamma").write_bytes(packer.inject(template, image))
        (self.root / "packing.stderr").write_bytes(b"")
        raw = gamma_observation(POSITIVE_ASSEMBLY)
        for ordinal in (0, 1):
            (self.root / f"execution-{ordinal}.raw").write_bytes(raw)
            (self.root / f"execution-{ordinal}.stderr").write_bytes(b"")
        for stage in driver.STAGES:
            write_marker(self.root, stage)

    def test_prepare_is_fresh_exact_and_pending(self) -> None:
        plan = driver.load_plan(self.root)
        self.assertEqual(plan["schema"], driver.PLAN_SCHEMA)
        self.assertEqual(
            plan["inputs"]["source_image"]["sha256"],
            "a0ecad14670247857e300b5539e0058d8f72054f92fabd1645fc4457b0ac53c9",
        )
        for name in ("run-elaboration.sh", "run-execution-0.sh", "run-execution-1.sh"):
            self.assertTrue((self.root / name).stat().st_mode & 0o100)
        result = self.run_cli("status")
        self.assertEqual(result.returncode, 3, result.stderr)
        status = json.loads(result.stdout)
        self.assertFalse(status["ready_to_finalize"])
        self.assertTrue(all(row["state"] == "pending" for row in status["stages"].values()))
        duplicate = subprocess.run(
            [sys.executable, "-B", str(HERE / "lower_rooted_assembly_publication_v1_driver.py"),
             "prepare", str(self.root)],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
        )
        self.assertEqual(duplicate.returncode, 2)
        self.assertEqual(duplicate.stdout, b"")

    def test_complete_status_and_finalize_exact_receipt(self) -> None:
        self.materialize_completed_outputs()
        state = self.run_cli("status")
        self.assertEqual(state.returncode, 0, state.stderr)
        self.assertTrue(json.loads(state.stdout)["ready_to_finalize"])
        finalized = self.run_cli("finalize")
        self.assertEqual(finalized.returncode, 0, finalized.stderr)
        receipt = json.loads(finalized.stdout)
        self.assertEqual(receipt["claim"], publication.CLAIM)
        self.assertEqual((self.root / "receipt.json").read_bytes(), finalized.stdout)
        self.assertEqual(
            (self.root / "execution-0.s").read_bytes(),
            (self.root / "execution-1.s").read_bytes(),
        )
        again = self.run_cli("finalize")
        self.assertEqual(again.returncode, 251)
        self.assertEqual(again.stdout, b"")

    def test_stale_token_and_epoch_reject_without_receipt(self) -> None:
        self.materialize_completed_outputs()
        path = self.root / "execution-1.started.json"
        value = json.loads(path.read_bytes())
        value["attempt_id"] = "0" * 64
        path.write_bytes(driver.canonical_json(value))
        result = self.run_cli("status")
        self.assertEqual(result.returncode, 251)
        self.assertEqual(result.stdout, b"")
        self.assertFalse((self.root / "receipt.json").exists())

        # Restore the token but move the stage before the attempt epoch.
        value["attempt_id"] = driver.load_plan(self.root)["attempt_id"]
        value["start_epoch_ns"] = driver.load_plan(self.root)["prepared_epoch_ns"] - 1
        path.write_bytes(driver.canonical_json(value))
        result = self.run_cli("status")
        self.assertEqual(result.returncode, 251)
        self.assertEqual(result.stdout, b"")

    def test_cross_pair_output_and_failed_status_do_not_finalize(self) -> None:
        self.materialize_completed_outputs()
        (self.root / "execution-1.raw").write_bytes(
            gamma_observation(POSITIVE_ASSEMBLY.replace(b"65,66,67", b"65,66,68"))
        )
        result = self.run_cli("status")
        self.assertEqual(result.returncode, 251)
        self.assertEqual(result.stdout, b"")
        self.assertFalse((self.root / "receipt.json").exists())

        # A coherent nonzero process result is incomplete evidence, not stale
        # evidence, and therefore reports not-ready rather than publishing.
        self.root = Path(self.temporary.name) / "failed"
        shutil.copytree(self.base, self.root)
        self.materialize_completed_outputs()
        finish = self.root / "execution-1.finished.json"
        value = json.loads(finish.read_bytes())
        value["status"] = 9
        finish.write_bytes(driver.canonical_json(value))
        result = self.run_cli("status")
        self.assertEqual(result.returncode, 3)
        self.assertEqual(json.loads(result.stdout)["stages"]["execution-1"]["state"], "failed")
        final = self.run_cli("finalize")
        self.assertEqual(final.returncode, 3)
        self.assertEqual(final.stdout, b"")

    def test_missing_finish_is_running_not_reused_status(self) -> None:
        plan = driver.load_plan(self.root)
        stage = "execution-0"
        (self.root / "closed.gamma").write_bytes(b"test-only pending input\n")
        inputs, _ = driver.stage_paths(self.root, stage)
        started = {
            "attempt_id": plan["attempt_id"],
            "inputs": driver.marker_identity(inputs, f"{stage}_input"),
            "prepared_epoch_ns": plan["prepared_epoch_ns"],
            "schema": driver.MARKER_SCHEMA,
            "stage": stage,
            "start_epoch_ns": max(plan["prepared_epoch_ns"] + 1, time.time_ns() - 1),
        }
        (self.root / f"{stage}.started.json").write_bytes(driver.canonical_json(started))
        result = self.run_cli("status")
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertEqual(json.loads(result.stdout)["stages"][stage]["state"], "running")


class UsageTests(unittest.TestCase):
    def test_relative_and_missing_arguments_reject(self) -> None:
        script = str(HERE / "lower_rooted_assembly_publication_v1_driver.py")
        for arguments in (("prepare", "relative"), ("status",)):
            result = subprocess.run(
                [sys.executable, "-B", script, *arguments],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
            )
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, b"")


if __name__ == "__main__":
    unittest.main()
