#!/usr/bin/env python3
# Runs every fixture in fixtures.tsv through the selected Gamma evaluator
# (MODE=selected, EVALUATOR=path) or through the untrusted Alpha reference
# interpreter over the selected evaluator tape (MODE=reference, REFERENCE and
# TAPE paths). Pins exit code and published output. Reference mode also
# reports Alpha instruction counts and pair allocations per fixture; those are
# diagnostics for the comparison, not evaluator acceptance.
#
# Selected mode additionally runs every delta_fixtures.tsv row: the Delta
# source is admitted through the canonical selected Delta compiler (DCREQ
# profile 1) under the same evaluator, which is the customer's own static
# nominal-typing implementation. Compile status and output pin exactly
# (sha256 for receipts, exact bytes for DCOUT failure frames); an accepted
# receipt is then executed and its published bytes pinned. Reference mode
# skips that leg: the compiler is far too large for the untrusted
# instruction-counting interpreter.
import hashlib
import os
import signal
import struct
import subprocess
import sys
from pathlib import Path

GATE_DIR = Path(os.environ["GATE_DIR"])
MODE = os.environ["MODE"]
PAIR_RECORD = 0x28
PAIR_HEAP_BASE = 0x10000000
DCOUT_MAGIC = b"\xffDCOUT\x01\x00"


def load_fixtures():
    rows = []
    for line in (GATE_DIR / "fixtures.tsv").read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        name, exit_code, published = line.split("\t")
        rows.append((name, int(exit_code), bytes.fromhex(published)))
    return rows


def load_delta_fixtures():
    path = GATE_DIR / "delta_fixtures.tsv"
    if not path.exists():
        return []
    rows = []
    for line in path.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        rows.append(line.split("\t"))
    return rows


def request(program, sealed=b""):
    return struct.pack("<I", len(program)) + program + sealed


def run_selected(program, sealed=b"", timeout=20):
    process = subprocess.Popen(
        [os.environ["EVALUATOR"]], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, _ = process.communicate(request(program, sealed), timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("Gamma evaluation timed out")
    return process.returncode, output, None


def dcreq(profile, delta_source, support):
    return (
        b"DCREQ\x01\x00\x00"
        + struct.pack("<I", profile)
        + struct.pack("<I", len(delta_source))
        + delta_source
        + support
    )


def output_matches(spec, output):
    if spec.startswith("hex:"):
        return output.hex() == spec[4:]
    if spec.startswith("sha256:"):
        return hashlib.sha256(output).hexdigest() == spec[7:]
    raise SystemExit(f"delta_fixtures.tsv: unknown output spec {spec!r}")


def describe_output(output):
    if output.startswith(DCOUT_MAGIC) and len(output) == 40:
        return (
            f"DCOUT tag={output[8]} space={output[9]} "
            f"code={int.from_bytes(output[12:16], 'little')} "
            f"coordinate={int.from_bytes(output[16:24], 'little')}"
        )
    return f"{len(output)} bytes sha256={hashlib.sha256(output).hexdigest()}"


def run_reference(source):
    # Instrument alpha_ref.py's small-step loop without maintaining a copy:
    # count fetched opcodes and read the pair heap cursor (ra0) at halt or trap.
    text = Path(os.environ["REFERENCE"]).read_text()
    text = text.replace("        op = M[pc]\n", "        op = M[pc]\n        COUNTER[0] += 1\n", 1)
    text = text.replace("        if op == 0x00:", "        if op == 0x00:\n            COUNTER[1] = R[0xa0]", 1)
    text = text.replace("        sys.exit(132)", "        COUNTER[1] = R[0xa0]\n        sys.exit(132)", 1)
    text = text.replace("\nmain()\n", "\n")
    assert text.count("COUNTER") == 3, "alpha_ref.py loop shape changed"
    process = subprocess.Popen(
        [sys.executable, "-c", REFERENCE_DRIVER, os.environ["TAPE"]],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        env={**os.environ, "REFERENCE_TEXT": text},
    )
    output, diagnostics = process.communicate(request(source), timeout=600)
    words = diagnostics.split()
    steps, heap = (int(words[0]), int(words[1])) if len(words) >= 2 else (-1, -1)
    pairs = (heap - PAIR_HEAP_BASE) // PAIR_RECORD if heap >= PAIR_HEAP_BASE else -1
    return process.returncode, output, (steps, pairs)


REFERENCE_DRIVER = r"""
import atexit, os, sys
COUNTER = [0, 0]
namespace = {"COUNTER": COUNTER, "__name__": "alpha_ref_counted"}
exec(compile(os.environ["REFERENCE_TEXT"], "alpha_ref.py", "exec"), namespace)
atexit.register(lambda: sys.stderr.write("%d %d\n" % (COUNTER[0], COUNTER[1])))
namespace["main"]()
"""


def run_delta_fixtures():
    rows = load_delta_fixtures()
    if not rows:
        return 0
    if MODE != "selected":
        print("delta fixtures skipped: canonical Delta compilation needs the "
              "selected evaluator; reference mode is a diagnostic interpreter")
        return 0
    compiler = Path(os.environ["DELTA_COMPILER"]).read_bytes()
    support = Path(os.environ["DELTA_SUPPORT"]).read_bytes()
    failures = 0
    for name, compile_exit, compile_spec, run_exit, run_spec in rows:
        source = (GATE_DIR / name).read_bytes()
        status, output, _ = run_selected(
            compiler, dcreq(1, source, support), timeout=30
        )
        detail = f"{len(source)} bytes source, {describe_output(output)}"
        verdict = "ok" if (
            status == int(compile_exit) and output_matches(compile_spec, output)
        ) else "FAIL"
        if verdict == "ok" and run_spec != "-":
            run_status, run_output, _ = run_selected(output)
            detail += f", receipt publishes {run_output.hex()}"
            if (run_status, run_output) != (int(run_exit), bytes.fromhex(run_spec[4:])):
                verdict = "FAIL"
                detail += f" (expected exit {run_exit} output {run_spec[4:]})"
        if verdict == "FAIL":
            failures += 1
            detail += (
                f", wanted compile exit {compile_exit} output {compile_spec}"
            )
        print(f"{verdict} {name}: {detail}")
    return failures


def main():
    runner = run_selected if MODE == "selected" else run_reference
    failures = 0
    if MODE == "reference":
        print("reference mode: UNTRUSTED alpha_ref.py over the selected tape; "
              "diagnostic only, not selected-evaluator acceptance")
    for name, expected_exit, expected_output in load_fixtures():
        source_path = GATE_DIR / name
        source = source_path.read_bytes()
        exit_code, output, diagnostics = runner(source)
        definitions = source.count(b"(def ")
        detail = f"{len(source)} bytes, {definitions} defs"
        if diagnostics is not None:
            steps, pairs = diagnostics
            detail += f", {steps} alpha steps, {pairs} pairs allocated"
        verdict = "ok" if (exit_code, output) == (expected_exit, expected_output) else "FAIL"
        if verdict == "FAIL":
            failures += 1
            detail += f", got exit {exit_code} output {output.hex()}"
        print(f"{verdict} {name}: exit {expected_exit}, output {expected_output.hex()}; {detail}")
    failures += run_delta_fixtures()
    if failures:
        raise SystemExit(f"Gamma product comparison: {failures} fixture(s) disagreed")
    print("Gamma product comparison: all fixtures agree")


main()
