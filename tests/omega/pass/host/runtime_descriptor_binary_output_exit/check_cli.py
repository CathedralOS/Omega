"""Pin this fixture's byte handoff through the public command, not Rust IR tests."""

import argparse
import os
from pathlib import Path
import platform
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--omega", default="omega", help="fresh compiler binary")
    arguments = parser.parse_args()
    if sys.platform != "darwin" or platform.machine() not in ("arm64", "aarch64"):
        parser.error("this fixture requires a matching macOS AArch64 host")
    fixture = Path(__file__).resolve().parent
    repository = fixture.parents[4]
    environment = dict(os.environ)
    environment["OMEGA_LIBRARY_ROOT"] = str(repository / "source/library")
    command = [arguments.omega, "run", str(fixture / "main.omg")]
    expected_output = bytes([255, 0, 128, 10])
    for compare_interpreter in (False, True):
        invocation = command + (["--both"] if compare_interpreter else [])
        result = subprocess.run(invocation, stdin=subprocess.DEVNULL,
                                capture_output=True, env=environment, timeout=90)
        assert result.returncode == 0, (result.returncode, result.stderr)
        assert result.stdout == expected_output, result.stdout.hex()
        expected_error = bytes([0, 254, 129, 10]) + b"native exit 0\n"
        if compare_interpreter:
            expected_error += b"interp exit: 0\n"
        assert result.stderr == expected_error, result.stderr.hex()
    for closed_stream in ("stdout", "stderr"):
        with subprocess.Popen(command, stdin=subprocess.DEVNULL,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              env=environment) as child:
            # Close before compilation finishes: the command's byte handoff
            # fails deterministically, not in a race with a reading consumer.
            getattr(child, closed_stream).close()
            setattr(child, closed_stream, None)
            try:
                output, error = child.communicate(timeout=90)
            except subprocess.TimeoutExpired:
                child.kill()
                child.communicate()
                raise
            assert child.returncode == 200, (child.returncode, output, error)
            if closed_stream == "stdout":
                assert b"cannot forward program output" in error, error
                assert b"panicked" not in error, error
            else:
                # No diagnostic can survive a closed stderr. The status still
                # reports failure, after the intact stdout was forwarded.
                assert output == expected_output, output.hex()
    print("CLI binary-output checks: 4 invocations passed")


if __name__ == "__main__":
    main()
