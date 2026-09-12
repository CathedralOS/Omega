#!/usr/bin/env python3
# Runs every fixture in fixtures.tsv through the selected Gamma evaluator
# (MODE=selected, EVALUATOR=path) or through the untrusted Alpha reference
# interpreter over the selected evaluator tape (MODE=reference, REFERENCE and
# TAPE paths). Pins exit code and published output. Reference mode also
# reports Alpha instruction counts and pair allocations per fixture; those are
# diagnostics for the comparison, not evaluator acceptance.
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


def load_fixtures():
    rows = []
    for line in (GATE_DIR / "fixtures.tsv").read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        name, exit_code, published = line.split("\t")
        rows.append((name, int(exit_code), bytes.fromhex(published)))
    return rows


def request(source):
    return struct.pack("<I", len(source)) + source


def run_selected(source):
    process = subprocess.Popen(
        [os.environ["EVALUATOR"]], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, _ = process.communicate(request(source), timeout=20)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("Gamma evaluation timed out")
    return process.returncode, output, None


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
    if failures:
        raise SystemExit(f"Gamma product comparison: {failures} fixture(s) disagreed")
    print("Gamma product comparison: all fixtures agree")


main()
