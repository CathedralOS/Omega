"""Shared evaluator request for the epsilon gates: one sealed program with a
length prefix, answered on stdout; a watchdog timeout is no language judgment.
"""


import os
import signal
import struct
import subprocess
import time


def run(evaluator, program, sealed_input, timeout, timeout_message):
    """Start the evaluator, deliver the request, and collect its answer as
    (returncode, stdout, stderr, elapsed). A timeout raises SystemExit."""
    started = time.monotonic()
    process = subprocess.Popen(
        [str(evaluator)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, start_new_session=True)
    try:
        output, error = process.communicate(
            struct.pack("<I", len(program)) + program + sealed_input,
            timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                           check=False)
        else:
            os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(timeout_message)
    return process.returncode, output, error, time.monotonic() - started
