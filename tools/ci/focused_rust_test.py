"""Compile or run one selected Rust test through Cargo and the build slot.

Compilation can finish and be cached before a red or interrupted test loses its
job. A later execution still asks Cargo to validate the current source, features
and profile; cached executables are never launched directly.
"""

import argparse
import os
import re
import subprocess
import sys


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--prepare", action="store_true", help="compile without executing tests")
    mode.add_argument("--validate", action="store_true", help="validate selection without tools")
    arguments = parser.parse_args(argv)
    package = os.environ["TEST_PACKAGE"]
    selected = os.environ["TEST_FILTER"]
    target = os.environ["TEST_TARGET"]
    if not re.fullmatch(r"[A-Za-z0-9_-]+", package):
        raise SystemExit("Select one workspace package")
    if not re.fullmatch(r"[A-Za-z0-9_:]+", selected):
        raise SystemExit("Select a nonempty Rust test name or module")
    if target and not re.fullmatch(r"[A-Za-z0-9_-]+", target):
        raise SystemExit("Invalid integration test target")
    if arguments.validate:
        return
    command = ["cargo", "test", "--locked", "-p", package]
    command += ["--test", target] if target else ["--lib"]
    if arguments.prepare:
        command += ["--no-run", selected]
    else:
        command += [selected, "--", "--nocapture", "--test-threads=1"]
    print("Source:", os.environ["GITHUB_SHA"], flush=True)
    print("Command:", command, flush=True)
    subprocess.run(["rustc", "--version", "--verbose"], check=True)
    process = subprocess.Popen(
        [sys.executable, "tools/build_slot.py", "--", *command],
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, text=True)
    passed = 0
    for line in process.stdout:
        print(line, end="", flush=True)
        result = re.search(r"test result: ok\. (\d+) passed;", line)
        if result:
            passed += int(result.group(1))
    status = process.wait()
    if status:
        raise SystemExit(status)
    if arguments.prepare:
        print("Compilation completed; no tests were executed.", flush=True)
    elif passed == 0:
        raise SystemExit("The filter executed no passing tests")


if __name__ == "__main__":
    main()
