#!/usr/bin/env python3
"""Run the Omega construction application on its own frozen input on Alpha."""
import argparse
from functools import partial
import json
from pathlib import Path
import runpy
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "tests/bootstrap"))
from route import save_report

constructor = runpy.run_path(str(ROOT / "source/application/construction/construct.py"))
digest = constructor["digest"]


def verify_receipt(packet, receipt, tape, producer_hash):
    # construct.py records both the outer process exit and the Epsilon guest exit.
    framed = bytes(5) + tape
    expected = {
        "compiler_kind": "epsilon-executable", "compiler_sha256": producer_hash,
        "runner_sha256": producer_hash, "inputs_unchanged": True,
        "published": True, "exit": 0, "epsilon_exit": 0,
        "input_sha256": digest(packet), "tape_bytes": len(tape), "tape_sha256": digest(tape),
        "stdout_bytes": len(framed), "stdout_sha256": digest(framed),
    }
    if not isinstance(receipt, dict):
        raise ValueError("first-generation receipt must be an object")
    for key, value in expected.items():
        actual = receipt.get(key)
        if type(actual) is not type(value) or actual != value:
            raise ValueError(f"first-generation receipt disagrees at {key}")
    if not tape:
        raise ValueError("first-generation tape is empty")


def observe(directory, report, runner, packet, label):
    runner_hash = digest(runner.read_bytes())
    stage = {"stage": label, "runner_sha256": runner_hash, "input_sha256": digest(packet)}
    report["stages"].append(stage)
    save_report(directory, report)
    start = time.monotonic()
    result = subprocess.run([str(runner)], input=packet, capture_output=True)
    stage.update(exit=result.returncode, elapsed_seconds=time.monotonic() - start,
                 stdout_sha256=digest(result.stdout), stdout_bytes=len(result.stdout),
                 stderr_sha256=digest(result.stderr), stderr_bytes=len(result.stderr))
    save_report(directory, report)
    if digest(runner.read_bytes()) != runner_hash:
        raise ValueError(f"{label} executable changed during the run")
    if result.stderr:
        raise ValueError(f"{label} wrote to stderr")
    return result.returncode, result.stdout


def checked_output(run, runner, packet, label):
    status, tape = run(runner, packet, label)
    if status != 0 or not tape:
        raise ValueError(f"{label} exited {status} and wrote {len(tape)} bytes")
    return tape


def executable(directory, name, tape):
    path = directory / (name + ".tape")
    path.write_bytes(tape)
    runner = directory / name
    constructor["stamp_constructor"](path, runner)
    return runner


def check_compiler(directory, runner, run):
    def request(value):
        source = b"machine main() -> u8 { " + value + b" }\n"
        return constructor["encode"]([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])[0]

    for value in (42, 43):
        label = f"{runner.name}-return-{value}"
        tape = checked_output(run, runner, request(str(value).encode("ascii")), label)
        program = executable(directory, label, tape)
        if run(program, b"", label + "-run") != (value, b""):
            raise ValueError(f"{label} did not return {value} silently")
    # Malformed framing has its own status. Ill-typed source must reach the
    # compiler, not the framing, capacity or unsupported-lowering boundary.
    for suffix, packet, statuses in [
        ("trailing-byte", request(b"42") + b"\x00", (253,)),
        ("wrong-return-type", request(b"true"), (249, 247)),
    ]:
        label = runner.name + "-" + suffix
        status, output = run(runner, packet, label)
        if status not in statuses or output:
            raise ValueError(f"{label} exited {status} and wrote {len(output)} bytes")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="product.bin captured by construct.py")
    parser.add_argument("epsilon_compiler", type=Path, help="seed-built Epsilon construction executable")
    parser.add_argument("output", type=Path, help="fresh directory for tapes and evidence")
    parser.add_argument("--reuse-tape", type=Path,
                        help="reuse a previous --epsilon-compiler output and its matching input.result.json")
    args = parser.parse_args()
    packet = args.input.read_bytes()
    # Re-read the matching checkout once to rule out a substitute customer.
    # Every actual compiler invocation below receives the frozen packet.
    if packet.startswith(struct.pack("<II", 0, 2)):
        canonical, manifest = constructor["compiler_input"](with_build_sources=True)
    else:
        canonical, manifest = constructor["compiler_input"]()
    if canonical != packet:
        raise ValueError("input differs from the full product closure in this checkout")
    producer = args.epsilon_compiler.resolve(strict=True)
    producer_hash = digest(producer.read_bytes())
    directory = args.output.resolve()
    directory.mkdir(parents=True)
    report = {"input_sha256": digest(packet), "units": len(manifest["units"]),
              "epsilon_compiler_sha256": producer_hash, "stages": [], "verified": False}
    report["closed_set_version"] = manifest.get("closed_set_version", 1)
    report["build_units"] = len(manifest.get("build_units", []))
    run = partial(observe, directory, report)
    try:
        if args.reuse_tape:
            first = args.reuse_tape.read_bytes()
            receipt_path = args.input.with_suffix(".result.json")
            receipt_bytes = receipt_path.read_bytes()
            verify_receipt(packet, json.loads(receipt_bytes), first, producer_hash)
            report["reused_epsilon_receipt_sha256"] = digest(receipt_bytes)
        else:
            status, observation = run(producer, packet, "build-omega-1")
            if report["stages"][-1]["runner_sha256"] != producer_hash:
                raise ValueError("Epsilon executable changed after capture")
            first, outcome = constructor["epsilon_tape"](observation)
            report["stages"][-1].update(outcome)
            if status != 0 or not first or outcome.get("epsilon_exit") != 0:
                raise ValueError(f"Epsilon construction failed with process exit {status}, guest {outcome}")
        report["first_tape_sha256"] = digest(first)
        previous = first
        for number in (1, 2):
            runner = executable(directory, f"omega-{number}", previous)
            check_compiler(directory, runner, run)
            rebuilt = checked_output(run, runner, packet, f"omega-{number}-self-build")
            if number == 2:
                (directory / "omega-3.tape").write_bytes(rebuilt)
                if rebuilt != previous:
                    raise ValueError("Omega generations two and three differ")
            previous = rebuilt
        report.update(verified=True, reproduced_tape_sha256=digest(previous))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        report["failure"] = str(error)
        raise
    finally:
        save_report(directory, report)
    print(f"Omega reproduced {len(previous)} bytes, SHA-256 {digest(previous)}")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from error
