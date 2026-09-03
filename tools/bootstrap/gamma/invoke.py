#!/usr/bin/env python3
"""Frame and invoke an exact Gamma source through a selected evaluator."""

import argparse
import os
import signal
import struct
import subprocess
import tempfile
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evaluator", required=True)
    parser.add_argument("--source", required=True)
    parser.add_argument("--input")
    parser.add_argument("--output", required=True)
    parser.add_argument("--timeout", type=float, default=120)
    arguments = parser.parse_args()

    source = Path(arguments.source).read_bytes()
    if len(source) > 0xFFFFFFFF:
        return 3
    sealed_input = Path(arguments.input).read_bytes() if arguments.input else b""
    request = struct.pack("<I", len(source)) + source + sealed_input

    process = subprocess.Popen(
        [arguments.evaluator],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        output, _ = process.communicate(request, timeout=arguments.timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        return 3

    if process.returncode != 0:
        return process.returncode

    destination = Path(arguments.output)
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(dir=destination.parent)
    try:
        with os.fdopen(descriptor, "wb") as temporary:
            temporary.write(output)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_name, destination)
    except BaseException:
        Path(temporary_name).unlink(missing_ok=True)
        raise
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
