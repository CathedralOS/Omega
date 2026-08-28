#!/usr/bin/env python3
"""Run and observe one exact Delta V1 native-artifact realization."""

from __future__ import annotations

import ctypes
import errno
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import lower_rooted_artifact_custody_v1 as custody


class RunnerError(Exception):
    pass


class RunnerUsageError(RunnerError):
    pass


def fail(message: str) -> None:
    raise RunnerError(message)


def usage(message: str) -> None:
    raise RunnerUsageError(message)


def require_absolute(path: Path, context: str) -> None:
    if not path.is_absolute():
        usage(f"{context} path must be absolute")


def destination_exists(path: Path) -> bool:
    return os.path.lexists(path)


def snapshot_inputs(
    assembly: Path,
    clang: Path,
    linker: Path,
    sdk_settings: Path,
    libsystem: Path,
    compiler_runtime: Path,
) -> dict:
    return {
        "assembly": custody.file_identity(
            assembly,
            "darwin_arm64_assembly_stdout",
            custody.support.MAX_ASSEMBLY_BYTES,
        ),
        "toolchain": {
            "clang_driver": custody.file_identity(
                clang,
                "ambient_apple_clang_driver",
                custody.MAX_TOOLCHAIN_COMPONENT_BYTES,
            ),
            "compiler_runtime": custody.file_identity(
                compiler_runtime,
                "ambient_clang_runtime_archive",
                custody.MAX_TOOLCHAIN_COMPONENT_BYTES,
            ),
            "libsystem_stub": custody.file_identity(
                libsystem,
                "ambient_macos_sdk_libsystem_stub",
                custody.MAX_SDK_COMPONENT_BYTES,
            ),
            "linker": custody.file_identity(
                linker,
                "ambient_apple_linker",
                custody.MAX_TOOLCHAIN_COMPONENT_BYTES,
            ),
            "sdk_settings": custody.file_identity(
                sdk_settings,
                "ambient_macos_sdk_settings",
                custody.MAX_SDK_COMPONENT_BYTES,
            ),
        },
    }


def publish_exclusive(staging: Path, destination: Path) -> None:
    """Atomically rename one directory without replacing an existing path."""

    library = ctypes.CDLL(None, use_errno=True)
    source_raw = os.fsencode(staging)
    destination_raw = os.fsencode(destination)
    if sys.platform == "darwin":
        rename = library.renamex_np
        rename.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
        rename.restype = ctypes.c_int
        result = rename(source_raw, destination_raw, 0x00000004)  # RENAME_EXCL
    elif sys.platform.startswith("linux") and hasattr(library, "renameat2"):
        rename = library.renameat2
        rename.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        rename.restype = ctypes.c_int
        result = rename(-100, source_raw, -100, destination_raw, 1)  # RENAME_NOREPLACE
    else:
        fail("exclusive directory publication is unsupported on this host")
    if result == 0:
        return
    error = ctypes.get_errno()
    if error in (errno.EEXIST, errno.ENOTEMPTY):
        usage("destination appeared during realization")
    raise OSError(error, os.strerror(error), os.fspath(destination))


def realize(
    destination: Path,
    assembly: Path,
    clang: Path,
    linker: Path,
    sdk_settings: Path,
    libsystem: Path,
    compiler_runtime: Path,
) -> None:
    paths = (
        (destination, "destination"),
        (assembly, "assembly"),
        (clang, "clang"),
        (linker, "linker"),
        (sdk_settings, "SDK settings"),
        (libsystem, "libSystem stub"),
        (compiler_runtime, "compiler runtime"),
    )
    for path, context in paths:
        require_absolute(path, context)
    if destination_exists(destination):
        usage("destination already exists")
    parent = destination.parent
    if not parent.is_dir():
        usage("destination parent must exist")

    spelling = tempfile.mkdtemp(prefix=".delta-realization-v1.", dir=parent)
    staging = Path(spelling)
    installed = False
    try:
        artifact = staging / "delta-compiler"
        stdout = staging / "realization.stdout"
        stderr = staging / "realization.stderr"
        observation = staging / "realization-observation.json"
        command = custody.realization_command(
            clang, linker, sdk_settings.parent, artifact, assembly
        )
        before_inputs = snapshot_inputs(
            assembly, clang, linker, sdk_settings, libsystem, compiler_runtime
        )
        started = time.monotonic_ns()
        with stdout.open("wb") as stdout_stream, stderr.open("wb") as stderr_stream:
            try:
                result = subprocess.run(
                    command,
                    stdin=subprocess.DEVNULL,
                    stdout=stdout_stream,
                    stderr=stderr_stream,
                    check=False,
                    timeout=custody.REALIZATION_TIMEOUT_SECONDS,
                )
            except subprocess.TimeoutExpired:
                fail("realization timeout")
        elapsed_milliseconds = (time.monotonic_ns() - started) // 1_000_000

        observe = subprocess.run(
            [
                sys.executable,
                "-B",
                os.fspath(Path(custody.__file__).resolve()),
                "observe",
                str(result.returncode),
                str(elapsed_milliseconds),
                os.fspath(assembly),
                os.fspath(artifact),
                os.fspath(stdout),
                os.fspath(stderr),
                os.fspath(clang),
                os.fspath(linker),
                os.fspath(sdk_settings),
                os.fspath(libsystem),
                os.fspath(compiler_runtime),
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if observe.returncode != 0:
            detail = observe.stderr.decode("utf-8", errors="replace").strip()
            fail(f"custody observe rejected realization: {detail}")
        observed = json.loads(observe.stdout)
        after_inputs = snapshot_inputs(
            assembly, clang, linker, sdk_settings, libsystem, compiler_runtime
        )
        if (
            observed.get("assembly") != before_inputs["assembly"]
            or observed.get("toolchain") != before_inputs["toolchain"]
            or after_inputs != before_inputs
        ):
            fail("realization inputs changed during command or observation")
        observation.write_bytes(observe.stdout)
        artifact.chmod(0o755)
        stdout.chmod(0o644)
        stderr.chmod(0o644)
        observation.chmod(0o644)

        if destination_exists(destination):
            usage("destination appeared during realization")
        publish_exclusive(staging, destination)
        installed = True
    finally:
        if not installed and staging.exists():
            shutil.rmtree(staging)


def main(arguments: list[str]) -> int:
    if len(arguments) != 7:
        usage(
            "expected DESTINATION ASSEMBLY CLANG LINKER SDK_SETTINGS "
            "LIBSYSTEM_STUB COMPILER_RUNTIME"
        )
    realize(*map(Path, arguments))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (RunnerError, OSError, ValueError) as error:
        status = 2 if isinstance(error, RunnerUsageError) else 251
        print(f"Delta artifact realization V1: {error}", file=sys.stderr)
        raise SystemExit(status)
