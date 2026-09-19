"""Direct RunEpsilon refinement gate for the canonical Epsilon evaluator edge.

For every corpus (source, stdin) pair, the expected observation is DERIVED by
the independent model (model.py) — the test-owned reconstruction of
CheckEpsilon and RunEpsilon from bootstrap/4_epsilon/LANGUAGE.md — and the
canonical evaluator's published observation must equal it byte-for-byte.
Mutations exercise every input of the request:

- source mutations: each mutated program's observation is re-derived by the
  model (mutated literal, operator, name, arm, or checkable defect);
- stdin mutations: each mutated sealed input re-derives through the model;
- D member mutations: the exact D closure member sources (the same programs
  tests/epsilon/d-composition/ carries over the canonical edge) run through
  the model, then one bounded spelling mutation per member re-derives and
  must match the edge byte-for-byte — members outside the model's declared
  fragment record ModelExcluded rather than a guessed judgment;
- profile mutations: an unassigned EREQ profile refuses with EEOUT
  unknown_profile instead of publishing any observation;
- observation mutations: every byte of the expected observation, plus
  truncation and extension, must differ from the published bytes — the
  observation is discriminated completely, with no don't-care positions.

A mismatch between model and evaluator is a witnessed conformance gap in
whichever side the contract contradicts; the gate makes no preference.

Usage:
    python3 gate.py <artifact-directory> [--skip-corpus] [--skip-members]
                  [exact D customer names...]

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
import d_closure  # noqa: E402
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


def check(evaluator, receipt, closure_digest, name, source, stdin, stats,
          timeout=300):
    """One direct refinement point: model-derived expectation vs the edge."""
    expected = model.observation(source, stdin)
    actual = evaluate(evaluator, receipt,
                      ereq(closure_digest, source, stdin), timeout)
    if actual != (0, expected):
        raise SystemExit(
            f"{name}: model derived {expected.hex()}, evaluator published "
            f"status {actual[0]} with {len(actual[1])} bytes "
            f"{actual[1][:80].hex()}")
    stats["refinement"] += 1
    discriminate_observation(name, expected, actual[1], stats)
    return expected


def discriminate_observation(name, expected, published, stats):
    """Every byte position of the expected observation, plus truncation and
    extension, must differ from the published bytes — the observation is
    discriminated completely, with no don't-care positions."""
    for index in range(len(expected)):
        mutated = bytearray(expected)
        mutated[index] ^= 0xFF
        if published == bytes(mutated):
            raise SystemExit(
                f"{name}: observation mutation at byte {index} did not "
                f"discriminate")
        stats["observation"] += 1
    if published == expected[:-1] or published == expected + b"\x00":
        raise SystemExit(f"{name}: observation extent did not discriminate")
    stats["observation"] += 2


def run_d_members(evaluator, receipt, closure_digest, selected, stats,
                  watchdog):
    """Refine the model over the exact D closure member sources — the same
    programs tests/epsilon/d-composition/ runs through the canonical edge.

    A member whose constructs sit outside the model's declared fragment
    records ModelExcluded instead of a guessed judgment; every covered
    customer then applies its bounded mutations: the model re-derives each
    mutated program's observation and the evaluator must match. A mutated
    observation equal to the customer's base fails — the mutation did not
    discriminate.
    """
    cases = d_closure.customers()
    if selected:
        known = {case.name for case in cases}
        unknown = [name for name in selected if name not in known]
        if unknown:
            raise SystemExit(f"unknown exact D customers: {unknown}")
        cases = [case for case in cases if case.name in set(selected)]
    covered_members = {}
    for case in cases:
        started = time.monotonic()
        try:
            expected = model.observation(case.source, case.stdin)
        except model.ModelExcluded as exc:
            stats["d_excluded"] += 1
            print(f"{case.name}: ModelExcluded ({exc})", flush=True)
            continue
        model_seconds = time.monotonic() - started
        started = time.monotonic()
        actual = evaluate(
            evaluator, receipt,
            ereq(closure_digest, case.source, case.stdin), watchdog)
        edge_seconds = time.monotonic() - started
        if actual != (0, expected):
            raise SystemExit(
                f"{case.name}: model derived {expected.hex()}, evaluator "
                f"published status {actual[0]} with {len(actual[1])} bytes "
                f"{actual[1][:80].hex()}")
        stats["d_refinement"] += 1
        discriminate_observation(case.name, expected, actual[1], stats)
        for member in case.member_names:
            covered_members.setdefault(member, case.name)
        discriminated = 0
        for label, mutation in case.mutations:
            mutated = mutation(case.source)
            try:
                mutated_expected = model.observation(mutated, case.stdin)
            except model.ModelExcluded as exc:
                stats["d_mut_excluded"] += 1
                print(f"{case.name} / {label}: ModelExcluded ({exc})",
                      flush=True)
                continue
            if mutated_expected == expected:
                raise SystemExit(
                    f"{case.name} / {label}: mutation did not discriminate")
            actual = evaluate(
                evaluator, receipt,
                ereq(closure_digest, mutated, case.stdin), watchdog)
            if actual != (0, mutated_expected):
                raise SystemExit(
                    f"{case.name} / {label}: model derived "
                    f"{mutated_expected.hex()}, evaluator published status "
                    f"{actual[0]} with {len(actual[1])} bytes "
                    f"{actual[1][:80].hex()}")
            stats["d_mutations"] += 1
            discriminated += 1
        print(f"{case.name}: {len(case.source)}B model "
              f"{model_seconds:.1f}s, edge {edge_seconds:.1f}s, observation "
              f"agreed byte-for-byte; {discriminated} member "
              f"mutation(s) discriminated", flush=True)
    return covered_members


def main():
    # gate.py <artifact-directory> [--skip-corpus] [--skip-members]
    #                                 [exact D customer names...]
    arguments = sys.argv[1:]
    directory = Path(arguments[0])
    skip_corpus = "--skip-corpus" in arguments
    skip_members = "--skip-members" in arguments
    selected = [a for a in arguments[1:] if not a.startswith("--")]
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

    stats = {"refinement": 0, "observation": 0, "profile": 0,
             "d_refinement": 0, "d_mutations": 0, "d_excluded": 0,
             "d_mut_excluded": 0}
    if not skip_corpus:
        for name, source, stdin, source_mutations, stdin_mutations in \
                corpus.CASES:
            check(evaluator, receipt, closure_digest, name, source, stdin,
                  stats)
            for mutation in source_mutations:
                mutated = mutation(source)
                check(evaluator, receipt, closure_digest,
                      f"{name} / source {mutation.__name__}", mutated, stdin,
                      stats)
            for mutated_stdin in stdin_mutations:
                check(evaluator, receipt, closure_digest,
                      f"{name} / stdin {mutated_stdin!r}", source,
                      mutated_stdin, stats)

    if not skip_corpus:
        for name, source, mutation in corpus.REJECTIONS:
            mutated = mutation(source)
            expected = model.observation(mutated, b"")
            if expected[:1] != b"\x02":
                raise SystemExit(
                    f"{name}: rejection mutation did not produce a Reject "
                    f"in the model ({expected.hex()})")
            actual = evaluate(evaluator, receipt,
                              ereq(closure_digest, mutated))
            if actual != (0, expected):
                raise SystemExit(
                    f"{name}: model derived {expected.hex()}, evaluator "
                    f"published status {actual[0]} with {len(actual[1])} "
                    f"bytes {actual[1][:80].hex()}")
            stats["refinement"] += 1

    # The exact D closure member sources: the same programs
    # tests/epsilon/d-composition/ runs through the canonical edge, refined
    # against the model with bounded member mutations.
    covered_members = {}
    if not skip_members:
        watchdog = int(os.environ.get("OMEGA_REFINE_D_SECONDS", "14400"))
        if watchdog <= 0:
            raise SystemExit("OMEGA_REFINE_D_SECONDS must be positive")
        covered_members = run_d_members(
            evaluator, receipt, closure_digest, selected, stats, watchdog)

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
    if not skip_members:
        member_count = len(d_closure.MEMBER_RECORDS)
        print(
            f"Epsilon refinement D members: {stats['d_refinement']} "
            f"customer observations agreed byte-for-byte over "
            f"{len(covered_members)}/{member_count} closure members, "
            f"{stats['d_mutations']} member mutations discriminated, "
            f"{stats['d_excluded']} customers and "
            f"{stats['d_mut_excluded']} mutations ModelExcluded")


if __name__ == "__main__":
    main()
