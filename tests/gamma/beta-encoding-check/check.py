"""Run the complete Beta-encoding certificate through the selected chain.

Native hosts only; run.sh refuses elsewhere.  The theory section is emitted
by the real Gamma source inside the materialized evaluator (the same prepare()
the theory gate uses), the certificate request is reproduced byte-exactly by
the pinned host-side stepper production and re-verified against the recorded
wire identity, and the evaluator then runs the checked closure over the framed
request.  A tag-7 observation is the checker's own verdict; this gate compares
and measures -- it grants nothing.

usage: check.py PREPARED_DIRECTORY (producer.gamma, checker.gamma, evaluator)
"""

import hashlib
import struct
import subprocess
import sys
import threading
import time
from pathlib import Path

GATE_DIR = Path(__file__).resolve().parent
sys.path.append(str(GATE_DIR.parent / "beta-encoding-theory"))

import full_subject  # noqa: E402
import gate as theory_gate  # noqa: E402
from lexical import certificate, envelope, proposition, record  # noqa: E402
from stepper import Stepper, Theory  # noqa: E402

# Selected provisions from bootstrap/proofs/checker/CHECKING.md:
# 130 MiB request extent inside the 131 MiB evaluator frame, and the 2^26
# work counter every admission and derivation transition shares.
REQUEST_EXTENT = 136_314_880
WORK_PROVISION = 67_108_864

# No measured wall-clock figure exists for the complete check yet; the first
# native run pins it.  The bound is deliberately generous for the projected
# 45-52M work units, and a timeout is a failed observation, not a verdict.
CHECK_TIMEOUT = 21_600


def build_request(theory_bytes):
    """Reproduce the pinned full-subject request byte-exactly.

    theory_bytes is the evaluator-emitted package (already pinned by
    prepare()); the host-side stepper replay re-derives every proof row, and
    the emitted wire must hash to the recorded request identity before the
    check run starts.
    """
    started = time.monotonic()
    source = full_subject.bound_subject(
        "OMEGA_PATH_GAMMA_EVALUATOR_SOURCE", full_subject.SOURCE_IDENTITY)
    tape = full_subject.bound_subject(
        "OMEGA_PATH_GAMMA_EVALUATOR_TAPE", full_subject.TAPE_IDENTITY)
    theory = Theory(theory_bytes)
    stepper = Stepper(theory)
    subject = full_subject.source_tree(stepper, source)
    result_tape = stepper.byte_list(tape)
    left = stepper.intern(2, full_subject.ENCODE_FUNCTION,
                          (subject, stepper.word(full_subject.SOURCE_LIMIT),
                           stepper.word(full_subject.OUTPUT_LIMIT)))
    right = stepper.intern(1, full_subject.R_SUCCESS, (result_tape,))
    value, _ = stepper.prove(left)
    if value != right:
        raise SystemExit(
            "certificate check: derivation concluded a different right root")
    owners, left_ref, right_ref, witnesses, proofs = stepper.encode(left, right)
    owner_records = [record(*row) for row in owners]
    witness_records = [record(*row) for row in witnesses]
    proof_records = [record(*row) for row in proofs]
    request = envelope((theory_bytes,
                        proposition(owner_records, left_ref, right_ref),
                        certificate(witness_records, proof_records)))
    full_subject.check_wire(request, owners, left_ref, right_ref,
                            witnesses, proofs)
    digest = hashlib.sha256(request).hexdigest()
    expected = full_subject.RECORDED
    if (len(request), digest) != (expected["request_bytes"],
                                  expected["request_sha256"]):
        raise SystemExit(
            f"certificate check: request identity {len(request)}B/{digest} "
            f"differs from the recorded {expected['request_bytes']}B/"
            f"{expected['request_sha256']}")
    print(f"certificate check: request {len(request)} bytes, "
          f"sha256={digest}, proof_rows={len(proofs)} "
          f"({time.monotonic() - started:.3f}s production)", flush=True)
    return request


def check(evaluator, checker, request):
    """Frame the request for the materialized evaluator and require the
    checker's exact Checked observation under the selected provisions."""
    framed = struct.pack("<I", len(checker)) + checker + request
    if len(framed) > REQUEST_EXTENT:
        raise SystemExit(
            f"certificate check: framed {len(framed)} bytes outside the "
            f"{REQUEST_EXTENT}-byte admission extent")
    started = time.monotonic()
    try:
        result = subprocess.run(
            [str(evaluator)], input=framed, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, timeout=CHECK_TIMEOUT)
    except subprocess.TimeoutExpired:
        raise SystemExit(
            f"certificate check: host timeout {CHECK_TIMEOUT}s; "
            "no checker result")
    elapsed = time.monotonic() - started
    if result.returncode != 0 or result.stderr:
        raise SystemExit(
            f"certificate check: evaluator failed status={result.returncode} "
            f"stderr={result.stderr!r}")
    output = result.stdout
    if len(output) == 33 and output[0] != 7:
        code, coordinate, limit, requested = struct.unpack_from("<4Q", output, 1)
        raise SystemExit(
            f"certificate check: checker refused tag={output[0]} code={code} "
            f"coordinate={coordinate} limit={limit} requested={requested}")
    expected_rows = full_subject.RECORDED["proof_rows"]
    if len(output) != 17 or output[0] != 7:
        raise SystemExit(
            f"certificate check: expected a 17-byte Checked observation, got "
            f"{len(output)} bytes "
            f"sha256={hashlib.sha256(output).hexdigest()}")
    count, work = struct.unpack_from("<2Q", output, 1)
    if count != expected_rows:
        raise SystemExit(
            f"certificate check: Checked count {count} differs from the "
            f"produced {expected_rows} proof rows")
    if not 0 < work <= WORK_PROVISION:
        raise SystemExit(
            f"certificate check: Checked work {work} outside the "
            f"{WORK_PROVISION}-unit provision")
    print(f"certificate check: Checked rows={count} work={work} "
          f"(provision {WORK_PROVISION}) elapsed={elapsed:.3f}s", flush=True)


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: check.py PREPARED_DIRECTORY "
                         "(producer.gamma, checker.gamma, evaluator)")
    temporary = Path(sys.argv[1]).resolve()
    failure = []

    def run():
        try:
            evaluator, _, checker, definitions = theory_gate.prepare(temporary)
            request = build_request(definitions)
            check(evaluator, checker, request)
        except BaseException as error:
            failure.append(error)

    # stepper.prove() recursion follows the derivation's premise depth; run on
    # a thread with an explicitly large stack like the production gate.
    try:
        threading.stack_size(512 * 1024 * 1024)
    except (ValueError, RuntimeError):
        pass
    worker = threading.Thread(target=run)
    worker.start()
    worker.join()
    if failure:
        raise failure[0]
    print("certificate check: complete certificate checked under the exact "
          "profile; mutation controls and rule retention remain open",
          flush=True)


if __name__ == "__main__":
    sys.setrecursionlimit(1_000_000)
    main()
