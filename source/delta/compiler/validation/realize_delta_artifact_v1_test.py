#!/usr/bin/env python3
"""Focused tests for the canonical initial Delta artifact realization runner."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from lower_rooted_artifact_custody_v1_test import minimal_macho
from publication_support_test import POSITIVE_ASSEMBLY


HERE = Path(__file__).resolve().parent
RUNNER = HERE / "realize-delta-artifact-v1.py"


class RealizationRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.destination = self.root / "result"
        self.assembly = self.root / "published.s"
        self.assembly.write_bytes(POSITIVE_ASSEMBLY)
        self.fixture = self.root / "fixture-macho"
        self.fixture.write_bytes(minimal_macho())
        self.arguments_log = self.root / "clang-arguments.json"
        self.clang = self.root / "fake-clang"
        self.linker = self.root / "ld"
        self.sdk = self.root / "MacOSX.sdk"
        self.sdk.mkdir()
        self.sdk_settings = self.sdk / "SDKSettings.json"
        self.libsystem = self.sdk / "libSystem.tbd"
        self.runtime = self.root / "libclang_rt.osx.a"
        for path, raw in (
            (self.linker, b"fake linker\n"),
            (self.sdk_settings, b'{"Version":"test"}\n'),
            (self.libsystem, b"fake libSystem\n"),
            (self.runtime, b"fake runtime\n"),
        ):
            path.write_bytes(raw)

    def write_clang(
        self,
        *,
        status: int = 0,
        stdout: str = "",
        stderr: str = "",
        create_destination: bool = False,
        mutate_runtime: bool = False,
    ) -> None:
        side_effects = ""
        if create_destination:
            side_effects += (
                f"destination = Path({str(self.destination)!r})\n"
                "destination.mkdir()\n"
                "(destination / 'owned').write_text('preserve\\n')\n"
            )
        if mutate_runtime:
            side_effects += f"Path({str(self.runtime)!r}).write_text('mutated runtime\\n')\n"
        program = f"""#!/usr/bin/env python3
import json
import shutil
import sys
from pathlib import Path
arguments = sys.argv[1:]
Path({str(self.arguments_log)!r}).write_text(json.dumps(arguments))
output = Path(arguments[arguments.index('-o') + 1])
shutil.copyfile({str(self.fixture)!r}, output)
{side_effects}
sys.stdout.write({stdout!r})
sys.stderr.write({stderr!r})
raise SystemExit({status})
"""
        self.clang.write_text(program)
        self.clang.chmod(0o755)

    def run_runner(self) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [
                sys.executable,
                "-B",
                str(RUNNER),
                str(self.destination),
                str(self.assembly),
                str(self.clang),
                str(self.linker),
                str(self.sdk_settings),
                str(self.libsystem),
                str(self.runtime),
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def assert_no_staging(self) -> None:
        self.assertEqual(list(self.root.glob(".delta-realization-v1.*")), [])

    def test_exact_command_and_atomic_four_file_result(self) -> None:
        self.write_clang()
        result = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(
            sorted(path.name for path in self.destination.iterdir()),
            [
                "delta-compiler",
                "realization-observation.json",
                "realization.stderr",
                "realization.stdout",
            ],
        )
        arguments = json.loads(self.arguments_log.read_text())
        self.assertEqual(
            arguments[:8],
            [
                "-arch", "arm64", "-isysroot", str(self.sdk),
                f"-fuse-ld={self.linker}", "-mmacosx-version-min=11.0",
                "-Wl,-no_uuid", "-Wl,-no_adhoc_codesign",
            ],
        )
        self.assertEqual(arguments[8], "-o")
        self.assertEqual(Path(arguments[9]).name, "delta-compiler")
        self.assertTrue(
            Path(arguments[9]).parent.name.startswith(".delta-realization-v1.")
        )
        self.assertEqual(arguments[10], str(self.assembly))
        observation = json.loads(
            (self.destination / "realization-observation.json").read_bytes()
        )
        self.assertEqual(observation["status"], 0)
        self.assertGreaterEqual(observation["elapsed_milliseconds"], 0)
        self.assertEqual((self.destination / "realization.stdout").read_bytes(), b"")
        self.assertEqual((self.destination / "realization.stderr").read_bytes(), b"")
        self.assert_no_staging()

    def test_nonzero_status_rejects_and_cleans_staging(self) -> None:
        self.write_clang(status=7)
        result = self.run_runner()
        self.assertEqual(result.returncode, 251, result.stderr)
        self.assertFalse(self.destination.exists())
        self.assert_no_staging()

    def test_stdout_and_stderr_diagnostics_reject_and_clean_staging(self) -> None:
        for stream in ("stdout", "stderr"):
            with self.subTest(stream=stream):
                self.write_clang(**{stream: "diagnostic\n"})
                result = self.run_runner()
                self.assertEqual(result.returncode, 251, result.stderr)
                self.assertFalse(self.destination.exists())
                self.assert_no_staging()

    def test_preexisting_destination_rejects_before_clang(self) -> None:
        self.write_clang()
        self.destination.mkdir()
        marker = self.destination / "owned"
        marker.write_text("preserve\n")
        result = self.run_runner()
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(marker.read_text(), "preserve\n")
        self.assertFalse(self.arguments_log.exists())
        self.assert_no_staging()

    def test_destination_created_by_clang_is_preserved_without_publish(self) -> None:
        self.write_clang(create_destination=True)
        result = self.run_runner()
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(
            sorted(path.name for path in self.destination.iterdir()), ["owned"]
        )
        self.assertEqual((self.destination / "owned").read_text(), "preserve\n")
        self.assert_no_staging()

    def test_mutated_realization_input_rejects_and_cleans_staging(self) -> None:
        self.write_clang(mutate_runtime=True)
        result = self.run_runner()
        self.assertEqual(result.returncode, 251, result.stderr)
        self.assertFalse(self.destination.exists())
        self.assert_no_staging()


if __name__ == "__main__":
    unittest.main()
