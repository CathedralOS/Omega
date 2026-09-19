"""Direct RunEpsilon refinement gate for the canonical Epsilon evaluator edge.

For every corpus (source, stdin) pair, the expected observation is DERIVED by
the independent model (model.py) — the test-owned reconstruction of
CheckEpsilon and RunEpsilon from bootstrap/4_epsilon/LANGUAGE.md — and the
canonical evaluator's published observation must equal it byte-for-byte.
Mutations exercise every input of the request:

- source mutations: each mutated program's observation is re-derived by the
  model (mutated literal, operator, name, arm, or checkable defect);
- stdin mutations: each mutated sealed input re-derives through the model;
- profile mutations: an unassigned EREQ profile refuses with EEOUT
  unknown_profile instead of publishing any observation;
- observation mutations: every byte of the expected observation, plus
  truncation and extension, must differ from the published bytes — the
  observation is discriminated completely, with no don't-care positions.

A mismatch between model and evaluator is a witnessed conformance gap in
whichever side the contract contradicts; the gate makes no preference.

Usage:
    python3 gate.py <artifact-directory>

The directory must contain evaluator.exe, epsilon_compiler.delta,
evaluator_entry.delta, delta_compiler.gamma, and support.bin (run.sh
materializes all of them). If canonical.gamma is absent it is reconstructed
through the bound Delta compiler and bound by identity afterward.
"""

import hashlib
import os
import signal
import struct
import subprocess
import sys
import time
from pathlib import Path

GATE_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(GATE_DIR))

import corpus  # noqa: E402
import model  # noqa: E402

EREQ = b"EEREQ\x01\x00\x00"
EEOUT = b"\xffEEOUT\x01\x00"

# Bound artifact identities (tools/bootstrap/epsilon/evaluator_env.sh owns
# the records; this gate restates them so a drifted input fails here).
EXPECTED = {
    "epsilon_compiler.delta": (
        617354,
        "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e"),
    "evaluator_entry.delta": (
        10950,
        "52032438c1236f51095b761afcb3111df2ae2d73ac9be7e91883bbfbd273e5e3"),
    "canonical.gamma": (
        729060,
        "bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368"),
}


def require_identity(label, data):
    size, digest = EXPECTED[label]
    actual = hashlib.sha256(data).hexdigest()
    if (len(data), actual) != (size, digest):
        raise SystemExit(
            f"Epsilon refinement: {label} identity changed to "
            f"{len(data)} bytes, {actual}")


def evaluate(evaluator, program, sealed_input, timeout=300):
    """Run one evaluator request; a timeout is no language judgment."""
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
        raise SystemExit("Epsilon refinement: selected Gamma timed out")
    if error:
        raise SystemExit(f"unexpected evaluator stderr: {error!r}")
    return process.returncode, output


def ereq(closure_digest, source=b"", stdin=b"", profile=1):
    return (EREQ + struct.pack("<III", profile, len(source), len(stdin))
            + closure_digest + source + stdin)


def eeout(code, coordinate, limit=0, requested=0, outcome=1, space=1):
    frame = struct.pack("<8sBBHIQQQ", EEOUT, outcome, space, 0, code,
                        coordinate, limit, requested)
    assert len(frame) == 40
    return 0, frame


def check(evaluator, receipt, closure_digest, name, source, stdin, stats):
    """One direct refinement point: model-derived expectation vs the edge."""
    expected = model.observation(source, stdin)
    actual = evaluate(evaluator, receipt,
                      ereq(closure_digest, source, stdin))
    if actual != (0, expected):
        raise SystemExit(
            f"{name}: model derived {expected.hex()}, evaluator published "
            f"status {actual[0]} with {len(actual[1])} bytes "
            f"{actual[1][:80].hex()}")
    stats["refinement"] += 1
    # Observation mutations: every byte position, truncation, and extension
    # must discriminate — the published observation equals the model's and
    # nothing else.
    for index in range(len(expected)):
        mutated = bytearray(expected)
        mutated[index] ^= 0xFF
        if actual[1] == bytes(mutated):
            raise SystemExit(
                f"{name}: observation mutation at byte {index} did not "
                f"discriminate")
        stats["observation"] += 1
    if actual[1] == expected[:-1] or actual[1] == expected + b"\x00":
        raise SystemExit(f"{name}: observation extent did not discriminate")
    stats["observation"] += 2


def main():
    directory = Path(sys.argv[1])
    evaluator = directory / "evaluator.exe"
    closure = (directory / "epsilon_compiler.delta").read_bytes()
    entry = (directory / "evaluator_entry.delta").read_bytes()
    require_identity("epsilon_compiler.delta", closure)
    require_identity("evaluator_entry.delta", entry)
    closure_digest = hashlib.sha256(closure).digest()

    cached = directory / "canonical.gamma"
    if cached.exists():
        receipt = cached.read_bytes()
    else:
        compiler = (directory / "delta_compiler.gamma").read_bytes()
        support = (directory / "support.bin").read_bytes()
        subject = closure + entry
        request = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
                   + subject + support)
        started = time.monotonic()
        status, receipt = evaluate(
            evaluator, compiler, request, timeout=900)
        print(f"Epsilon refinement: canonical receipt reconstructed in "
              f"{time.monotonic() - started:.1f}s", flush=True)
        if status != 0:
            raise SystemExit(
                f"canonical entry compilation returned {status}")
        if not receipt.startswith(b"(def $application () Int 1)\n"):
            raise SystemExit("canonical receipt lost its application marker")
        cached.write_bytes(receipt)
    require_identity("canonical.gamma", receipt)

    stats = {"refinement": 0, "observation": 0, "profile": 0}
    for name, source, stdin, source_mutations, stdin_mutations in \
            corpus.CASES:
        check(evaluator, receipt, closure_digest, name, source, stdin, stats)
        for mutation in source_mutations:
            mutated = mutation(source)
            check(evaluator, receipt, closure_digest,
                  f"{name} / source {mutation.__name__}", mutated, stdin,
                  stats)
        for mutated_stdin in stdin_mutations:
            check(evaluator, receipt, closure_digest,
                  f"{name} / stdin {mutated_stdin!r}", source, mutated_stdin,
                  stats)

    for name, source, mutation in corpus.REJECTIONS:
        mutated = mutation(source)
        expected = model.observation(mutated, b"")
        if expected[:1] != b"\x02":
            raise SystemExit(
                f"{name}: rejection mutation did not produce a Reject in "
                f"the model ({expected.hex()})")
        actual = evaluate(evaluator, receipt,
                          ereq(closure_digest, mutated))
        if actual != (0, expected):
            raise SystemExit(
                f"{name}: model derived {expected.hex()}, evaluator "
                f"published status {actual[0]} with {len(actual[1])} bytes "
                f"{actual[1][:80].hex()}")
        stats["refinement"] += 1

    # Profile mutations: version 1 assigns exactly profile 1
    # (ExactConsoleV1). Every other profile refuses with EEOUT
    # unknown_profile at the profile field — never an observation.
    for profile in (0, 2, 3, 0xFFFFFFFF):
        request = ereq(closure_digest, profile=profile)
        actual = evaluate(evaluator, receipt, request)
        if actual != eeout(3, 8):
            raise SystemExit(
                f"profile {profile}: expected EEOUT unknown_profile, got "
                f"status {actual[0]} with {actual[1][:40].hex()}")
        stats["profile"] += 1

    print(
        f"Epsilon refinement: {stats['refinement']} model-derived "
        f"observations agreed byte-for-byte, {stats['observation']} "
        f"observation mutations discriminated, {stats['profile']} profile "
        f"mutations refused without an observation")


if __name__ == "__main__":
    main()
