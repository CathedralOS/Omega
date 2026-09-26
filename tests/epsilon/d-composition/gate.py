"""Run D customers once, checking literal bytes and independent refinement.

The canonical Epsilon edge owns execution. Literal observations pin the
composition contract; the independent Epsilon model checks the same result
and derives expectations for the separate source-mutation controls.
"""

import hashlib
import os
import signal
import struct
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "refinement"))
import model
from customers import MEMBER_RECORDS, customers

EREQ = b"EEREQ\x01\x00\x00"


def run_evaluator(evaluator, program, sealed_input, timeout, label):
    started = time.monotonic()
    print(f"{label}: started, {timeout}s watchdog", flush=True)
    process = subprocess.Popen(
        [str(evaluator)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, start_new_session=True)
    try:
        output, errors = process.communicate(
            struct.pack("<I", len(program)) + program + sealed_input,
            timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                           check=False)
        else:
            os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"{label}: timed out; no language judgment")
    elapsed = time.monotonic() - started
    if errors:
        raise SystemExit(
            f"{label}: unexpected stderr after {elapsed:.3f}s: {errors!r}")
    print(f"{label}: status {process.returncode}, {len(output)} bytes out, "
          f"{elapsed:.3f}s", flush=True)
    return process.returncode, output, elapsed


def require_identity(label, source, length, digest):
    actual = hashlib.sha256(source).hexdigest()
    if (len(source), actual) != (length, digest):
        raise SystemExit(f"{label}: identity changed to {len(source)} bytes, {actual}")


def main():
    directory = Path(sys.argv[1]).resolve()
    selected = sys.argv[2:]  # optional exact customer names
    cases = customers()
    if selected:
        known = {case.name for case in cases}
        unknown = [name for name in selected if name not in known]
        if unknown:
            raise SystemExit(f"unknown exact D customers: {unknown}")
        cases = [case for case in cases if case.name in set(selected)]

    epsilon = (directory / "epsilon_compiler.delta").read_bytes()
    require_identity("Epsilon evaluator closure", epsilon, 617354,
                     "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e")
    entry_source = (Path(os.environ["OMEGA_REPO_ROOT"]) /
                    "tests/epsilon/evaluator-entry/evaluator_entry.delta").read_bytes()
    require_identity("canonical entry", entry_source, 10950,
                     "52032438c1236f51095b761afcb3111df2ae2d73ac9be7e91883bbfbd273e5e3")

    subject = epsilon + entry_source
    support = (directory / "support.bin").read_bytes()
    request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
               + subject + support)
    status, receipt, _ = run_evaluator(
        directory / "evaluator.exe",
        (directory / "delta_compiler.gamma").read_bytes(), request,
        900, "canonical receipt reconstruction")
    if status != 0:
        raise SystemExit(f"canonical receipt reconstruction status {status}")
    require_identity("canonical evaluator receipt", receipt, 729060,
                     "bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368")

    closure_digest = hashlib.sha256(epsilon).digest()

    def ereq(source, stdin=b""):
        return (EREQ + struct.pack("<III", 1, len(source), len(stdin))
                + closure_digest + source + stdin)

    watchdog = int(os.environ.get("OMEGA_DCOMP_OBSERVATION_SECONDS", "14400"))
    if watchdog <= 0:
        raise SystemExit("OMEGA_DCOMP_OBSERVATION_SECONDS must be positive")

    covered_members = set()
    refined = mutations = excluded = excluded_mutations = 0
    for case in cases:
        status, observation, _ = run_evaluator(
            directory / "evaluator.exe", receipt, ereq(case.source, case.stdin),
            watchdog, case.name)
        if (status, observation) != (0, case.expected):
            raise SystemExit(
                f"{case.name}: expected status 0 and {case.expected[:60].hex()}, "
                f"received status {status} and {observation[:60].hex()}")
        print(f"{case.name}: literal observation exact", flush=True)

        try:
            expected = model.observation(case.source, case.stdin)
        except model.ModelExcluded as error:
            excluded += 1
            print(f"{case.name}: ModelExcluded ({error})", flush=True)
            continue
        if observation != expected:
            raise SystemExit(
                f"{case.name}: model derived {expected[:60].hex()}, "
                f"evaluator published {observation[:60].hex()}")
        refined += 1
        covered_members.update(case.member_names)
        for label, mutation in case.mutations:
            changed = mutation(case.source)
            try:
                expected_mutation = model.observation(changed, case.stdin)
            except model.ModelExcluded as error:
                excluded_mutations += 1
                print(f"{case.name} / {label}: ModelExcluded ({error})", flush=True)
                continue
            if expected_mutation == expected:
                raise SystemExit(f"{case.name} / {label}: mutation did not discriminate")
            status, observation, _ = run_evaluator(
                directory / "evaluator.exe", receipt, ereq(changed, case.stdin),
                watchdog, f"{case.name} / {label}")
            if (status, observation) != (0, expected_mutation):
                raise SystemExit(
                    f"{case.name} / {label}: model derived {expected_mutation[:60].hex()}, "
                    f"evaluator published status {status} and {observation[:60].hex()}")
            mutations += 1

    print(f"Epsilon D composition: {len(cases)} literal observations passed; "
          f"{refined} model comparisons over {len(covered_members)}/{len(MEMBER_RECORDS)} "
          f"members, {mutations} source mutations; {excluded} customers and "
          f"{excluded_mutations} mutations ModelExcluded", flush=True)


if __name__ == "__main__":
    main()
