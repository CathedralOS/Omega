#!/usr/bin/env python3
"""Reconstruct temporary Delta publication evidence and verify an installation."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

import lower_rooted_assembly_publication_v1 as publication
import lower_rooted_assembly_publication_v1_driver as driver
import publication_support as support


HERE = Path(__file__).resolve().parent
VERIFY_INSTALLED = HERE / "verify-installed-artifact-v1.sh"
VERIFY_TIMEOUT_SECONDS = 600


class ReconstructionError(Exception):
    pass


class ReconstructionUsageError(ReconstructionError):
    pass


def fail(message: str) -> None:
    raise ReconstructionError(message)


def usage(message: str) -> None:
    raise ReconstructionUsageError(message)


def require_absolute(path: Path, context: str) -> None:
    if not path.is_absolute():
        usage(f"{context} path must be absolute")


def receipt_elapsed(receipt_path: Path) -> tuple[int, tuple[int, int]]:
    receipt = publication.load_json(receipt_path, "installed assembly receipt")
    if (
        receipt.get("schema") != publication.RECEIPT_SCHEMA
        or receipt.get("publication_id") != publication.PUBLICATION_ID
        or receipt.get("receipt_sha256") != publication.receipt_digest(receipt)
    ):
        fail("installed assembly receipt identity")
    elaboration = receipt.get("elaboration")
    executions = receipt.get("executions")
    if not isinstance(elaboration, dict) or not isinstance(executions, list) or len(executions) != 2:
        fail("installed assembly receipt observation shape")
    elaboration_elapsed = elaboration.get("elapsed_milliseconds")
    if elaboration.get("status") != 0:
        fail("installed assembly receipt elaboration status")
    if (
        isinstance(elaboration_elapsed, bool)
        or not isinstance(elaboration_elapsed, int)
        or elaboration_elapsed < 0
    ):
        fail("installed assembly receipt elaboration elapsed")
    execution_elapsed: list[int] = []
    for ordinal, execution in enumerate(executions):
        if not isinstance(execution, dict):
            fail("installed assembly receipt execution shape")
        elapsed = execution.get("elapsed_milliseconds")
        if (
            execution.get("ordinal") != ordinal
            or execution.get("status") != 0
            or execution.get("semantic_status") != 0
            or isinstance(elapsed, bool)
            or not isinstance(elapsed, int)
            or elapsed < 0
        ):
            fail("installed assembly receipt execution status/elapsed")
        execution_elapsed.append(elapsed)
    for key in ("assembly", "gamma_stdout", "stderr"):
        if executions[0].get(key) != executions[1].get(key):
            fail("installed assembly receipt does not admit repeated-byte reconstruction")
    return elaboration_elapsed, (execution_elapsed[0], execution_elapsed[1])


def write_canonical(path: Path, value: dict) -> None:
    path.write_bytes(publication.canonical_json(value, pretty=True))


def reconstruct_and_verify(
    installation: Path,
    clang: Path,
    linker: Path,
    sdk_settings: Path,
    libsystem: Path,
    compiler_runtime: Path,
) -> None:
    for path, context in (
        (installation, "installation"),
        (clang, "clang"),
        (linker, "linker"),
        (sdk_settings, "SDK settings"),
        (libsystem, "libSystem stub"),
        (compiler_runtime, "compiler runtime"),
    ):
        require_absolute(path, context)
    if installation.name != "darwin-arm64-v1" or not installation.is_dir():
        usage("installation must be an existing darwin-arm64-v1 directory")

    assembly_receipt = installation / "assembly-publication-receipt.json"
    retained_raw = installation / "execution.raw"
    elaboration_elapsed, execution_elapsed = receipt_elapsed(assembly_receipt)
    raw = driver.bounded_read(
        retained_raw,
        "installed canonical Gamma observation",
        publication.MAX_GAMMA_OBSERVATION,
    )

    verified_stdout = b""
    with tempfile.TemporaryDirectory(prefix="omega-delta-install-reconstruct-") as spelling:
        root = Path(spelling)
        driver.build_tools(root)
        artifacts = root / "artifacts"
        assembler_tape = artifacts / "assembler.tape"
        translator_tape = artifacts / "delta2gamma.tape"
        interpreter_tape = artifacts / "interp.tape"

        image = support.materialize_canonical_image(
            driver.MANIFEST, driver.LOCATIONS, {"delta": driver.DELTA}
        )
        template = root / "template.gamma"
        template_raw = driver.run_short(
            [os.fspath(artifacts / "delta2gamma.exe")],
            image,
            "reconstruct Delta compiler Gamma template",
        )
        if len(template_raw) > publication.MAX_TEMPLATE:
            raise driver.DriverResourceError("reconstructed Gamma template ceiling")
        template.write_bytes(template_raw)

        packer = publication._load_module(
            "delta_install_reconstruction_packer", publication.PACKER_SOURCE
        )
        gamma_raw = packer.inject(template_raw, image)
        if len(gamma_raw) > publication.MAX_CLOSED_GAMMA:
            raise driver.DriverResourceError("reconstructed closed Gamma ceiling")
        gamma = root / "closed.gamma"
        gamma.write_bytes(gamma_raw)

        decoder = publication._load_module(
            "delta_install_reconstruction_decoder", publication.DECODER_SOURCE
        )
        try:
            semantic_status, assembly_raw = decoder.decode(raw.decode("ascii"))
        except Exception as error:
            fail(f"installed Gamma observation decode: {error}")
        if semantic_status != 0:
            fail("installed Gamma observation semantic status")
        assembly = root / "execution.s"
        assembly.write_bytes(assembly_raw)
        empty = root / "empty"
        empty.write_bytes(b"")

        elaboration_observation = root / "elaboration.json"
        write_canonical(
            elaboration_observation,
            publication.make_elaboration_observation(
                0,
                elaboration_elapsed,
                driver.MANIFEST,
                driver.LOCATIONS,
                {"delta": driver.DELTA},
                assembler_tape,
                translator_tape,
                interpreter_tape,
                template,
                empty,
            ),
        )
        execution_observations: list[Path] = []
        for ordinal in (0, 1):
            observation = root / f"execution-{ordinal}.json"
            write_canonical(
                observation,
                publication.make_execution_observation(
                    ordinal,
                    0,
                    execution_elapsed[ordinal],
                    driver.MANIFEST,
                    driver.LOCATIONS,
                    {"delta": driver.DELTA},
                    assembler_tape,
                    translator_tape,
                    interpreter_tape,
                    template,
                    gamma,
                    retained_raw,
                    assembly,
                    empty,
                ),
            )
            execution_observations.append(observation)

        command = [
            os.fspath(VERIFY_INSTALLED),
            os.fspath(installation),
            os.fspath(empty),
            os.fspath(empty),
            os.fspath(clang),
            os.fspath(linker),
            os.fspath(sdk_settings),
            os.fspath(libsystem),
            os.fspath(compiler_runtime),
            os.fspath(assembler_tape),
            os.fspath(translator_tape),
            os.fspath(interpreter_tape),
            os.fspath(template),
            os.fspath(gamma),
            os.fspath(elaboration_observation),
            os.fspath(empty),
            os.fspath(execution_observations[0]),
            os.fspath(assembly),
            os.fspath(empty),
            os.fspath(execution_observations[1]),
            os.fspath(retained_raw),
            os.fspath(assembly),
            os.fspath(empty),
        ]
        try:
            result = subprocess.run(
                command,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
                timeout=VERIFY_TIMEOUT_SECONDS,
            )
        except subprocess.TimeoutExpired:
            fail("installed reconstruction verifier timeout")
        if result.returncode != 0 or result.stderr:
            detail = result.stderr.decode("utf-8", errors="replace").strip()
            fail(f"installed reconstruction verifier rejected: {detail}")
        verified_stdout = result.stdout
    sys.stdout.buffer.write(verified_stdout)


def main(arguments: list[str]) -> int:
    if len(arguments) != 6:
        usage(
            "expected INSTALLATION CLANG LINKER SDK_SETTINGS "
            "LIBSYSTEM_STUB COMPILER_RUNTIME"
        )
    reconstruct_and_verify(*map(Path, arguments))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (
        ReconstructionError,
        driver.DriverError,
        publication.ReceiptError,
        support.PublicationSupportError,
        OSError,
        UnicodeError,
        ValueError,
    ) as error:
        status = 2 if isinstance(error, ReconstructionUsageError) else getattr(error, "status", 251)
        print(f"Delta installed reconstruction V1: {error}", file=sys.stderr)
        raise SystemExit(status)
