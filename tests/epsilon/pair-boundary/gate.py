"""Observe the canonical Epsilon evaluator edge at its pair-arena boundary.

The cumulative immutable pair arena is the Gamma evaluator's private
40,265,318-node counter. Every Epsilon checking or execution allocation on the
canonical receipt is a node in that one arena, so an Epsilon program that
allocates without bound must end as the section-10 outer Incomplete(pair
nodes, 40265318, 40265319): raw status 252 with empty stdout, never a
canonical observation and never an EEOUT frame. This gate executes that
refusal directly against the canonical receipt: the same sparse-write loop
runs bounded on the admitted side and unbounded on the refused side.
"""

import argparse
import csv
import hashlib
import os
import signal
import struct
import subprocess
import time
from pathlib import Path

EREQ = b"EEREQ\x01\x00\x00"

# Bound artifact identities (tools/bootstrap/epsilon/evaluator_env.sh owns
# the records; this gate restates them so a drifted input fails here).
EXPECTED = {
    "epsilon_compiler.delta": (
        617354, "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e"),
    "evaluator_entry.delta": (
        10950, "52032438c1236f51095b761afcb3111df2ae2d73ac9be7e91883bbfbd273e5e3"),
    "canonical.gamma": (
        729060, "bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368"),
}


def require_identity(label, data):
    size, digest = EXPECTED[label]
    actual = hashlib.sha256(data).hexdigest()
    if (len(data), actual) != (size, digest):
        raise SystemExit(
            f"Epsilon pair boundary: {label} identity changed to "
            f"{len(data)} bytes, {actual}")


def evaluate(evaluator, program, sealed_input, timeout, label):
    """Run one evaluator request; a timeout is no language judgment."""
    print(f"Epsilon pair boundary {label}: started; watchdog {timeout}s",
          flush=True)
    started = time.monotonic()
    process = subprocess.Popen(
        [str(evaluator)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, start_new_session=True)
    try:
        output, error = process.communicate(
            struct.pack("<I", len(program)) + program + sealed_input,
            timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(
            f"Epsilon pair boundary {label}: timed out; no language judgment")
    elapsed = time.monotonic() - started
    print(
        f"Epsilon pair boundary {label}: status {process.returncode}, "
        f"{len(output)} stdout bytes, {elapsed:.3f}s", flush=True)
    return process.returncode, output, error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path,
                        help="materialized artifact directory")
    parser.add_argument("--case",
                        help="exact fixture filename, including .epsilon")
    options = parser.parse_args()
    gate = Path(__file__).resolve().parent
    try:
        timeout = int(os.environ.get("OMEGA_EPSILON_PAIR_SECONDS", "7200"))
    except ValueError:
        raise SystemExit("OMEGA_EPSILON_PAIR_SECONDS must be a positive integer")
    if timeout <= 0:
        raise SystemExit("OMEGA_EPSILON_PAIR_SECONDS must be a positive integer")

    directory = options.directory
    evaluator = directory / "evaluator.exe"
    closure = (directory / "epsilon_compiler.delta").read_bytes()
    entry = (directory / "evaluator_entry.delta").read_bytes()
    require_identity("epsilon_compiler.delta", closure)
    require_identity("evaluator_entry.delta", entry)
    closure_digest = hashlib.sha256(closure).digest()

    # Reconstruct the canonical receipt through the bound Delta compiler:
    # packed closure + canonical entry + bound support section. The receipt's
    # own identity is checked before any Epsilon request runs.
    compiler = (directory / "delta_compiler.gamma").read_bytes()
    support = (directory / "support.bin").read_bytes()
    subject = closure + entry
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + support)
    status, receipt, error = evaluate(
        evaluator, compiler, request, 900, "canonical receipt reconstruction")
    if error or status != 0:
        raise SystemExit(
            f"Epsilon pair boundary: receipt reconstruction returned "
            f"{status} with stderr {error!r}")
    require_identity("canonical.gamma", receipt)

    with (gate / "fixtures.tsv").open(encoding="ascii", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if reader.fieldnames != ["fixture", "bytes", "sha256", "status",
                                 "stdout_hex"]:
            raise SystemExit("Epsilon pair boundary: invalid fixtures.tsv header")
        controls = {}
        for row in reader:
            name = row["fixture"]
            if (Path(name).name != name or not name.endswith(".epsilon")
                    or name in controls):
                raise SystemExit(
                    f"Epsilon pair boundary: invalid fixture identity {name}")
            source = (gate / name).read_bytes()
            actual = hashlib.sha256(source).hexdigest()
            if (len(source), actual) != (int(row["bytes"]), row["sha256"]):
                raise SystemExit(
                    f"Epsilon pair boundary: {name} identity changed")
            controls[name] = (source, int(row["status"]),
                              bytes.fromhex(row["stdout_hex"]))
    if not controls or set(controls) != {
            path.name for path in gate.glob("*.epsilon")}:
        raise SystemExit(
            "Epsilon pair boundary: fixture inventory does not cover the "
            "exact .epsilon inventory")
    if options.case is not None:
        if options.case not in controls:
            raise SystemExit(
                f"Epsilon pair boundary: unknown fixture {options.case!r}")
        controls = {options.case: controls[options.case]}

    for name, (source, status, stdout) in controls.items():
        # EREQ v1: ExactConsoleV1 profile, exact source section, empty stdin,
        # the bound closure identity. The host frames bytes only.
        ereq = (EREQ + struct.pack("<III", 1, len(source), 0)
                + closure_digest + source)
        actual = evaluate(evaluator, receipt, ereq, timeout, name)
        if actual != (status, stdout, b""):
            raise SystemExit(
                f"Epsilon pair boundary {name}: expected status {status} "
                f"and stdout {stdout.hex()}, got status {actual[0]}, "
                f"{len(actual[1])} stdout bytes {actual[1][:40].hex()}, "
                f"stderr {actual[2]!r}")
    print(
        f"Epsilon pair boundary: {len(controls)} case(s) pass; the refused "
        f"case is Incomplete(pair nodes, 40265318, 40265319) via status 252 "
        f"with empty stdout", flush=True)


if __name__ == "__main__":
    main()
