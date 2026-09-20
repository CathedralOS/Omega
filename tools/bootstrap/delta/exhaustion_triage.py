#!/usr/bin/env python3
"""Attribute a witnessed Delta compile/run observation to its allocation owner.

Investigation entry point, not a semantic stage: when a DCREQ compilation or a
generated-program execution ends in something that looks like exhaustion, this
script classifies the raw observation (exit status, stdout bytes, stderr bytes,
host timeout) before any deeper tracing. The discipline it encodes is the one
TASKS_BOOTSTRAP.md records for DELTA-COMPILER: a Gamma-evaluator-owned failure
is not a compiler DCOUT outcome, and a host watchdog kill is not a compiler
capacity judgment at all.

The classification contract is derived from two owners' published vocabularies:

* Canonical compiler outcomes (bootstrap/3_delta/LANGUAGE.md): success is
  status 0 with the receipt bytes on stdout; every failure is status 1..3
  carrying one exact 40-byte DCOUT frame whose tag agrees with the halt value:

      bytes  0.. 7  magic/version        ff 44 43 4f 55 54 01 00  ("\\xffDCOUT")
      byte   8      halt tag             1 Reject, 2 Incomplete, 3 InternalFailure
      byte   9      coordinate space     0 none, 1 source, 2 payload,
                                        3 internal row, 4 DCREQ, 5 support
      bytes 10..11  reserved             0
      bytes 12..15  reason/resource code  u32 little-endian
      bytes 16..23  coordinate            u64 little-endian
      bytes 24..31  limit                 u64 little-endian
      bytes 32..39  requested             u64 little-endian

  Those frames are the only compiler-owned outcomes; the compiler never emits
  a raw evaluator status for an owned refusal.

* Evaluator / generated-program statuses (bootstrap/3_delta/LANGUAGE.md and
  bootstrap/2_gamma/EVALUATOR_PROFILE.md): when the evaluator hosting the
  compiler (or executing a produced receipt) is itself the failing owner, the
  observation is a bare process status — 132 Alpha illegal-instruction trap,
  248 InternalFailure, 249 AuthoredTrap, 250 StackExhausted,
  251 MemoryContainmentViolation, 252 HeapExhausted, 253 InputExtent,
  254 OutputExtent. Status 3 with empty stdout (no frame) is the evaluator's
  generated-function census preflight, also evaluator-owned. Status 1 with
  empty stdout is invalid Gamma source before application execution.

A real heap-exhaustion witness exists in the record: the 4,000,000-byte name
regression at b35db7dc1f ended raw status 252 with empty stdout/stderr —
heap exhaustion, not a canonical Incomplete. Conversely, every owned refusal
in tests/delta/resource-boundary ends status 1..3 with the exact 40-byte
frame. A killed run is the harness's watchdog, attributed to the host.

Usage:

    python3 tools/bootstrap/delta/exhaustion_triage.py --status 252
    python3 tools/bootstrap/delta/exhaustion_triage.py --status 2 \\
        --stdout-file observed.bin --stderr-file observed.err
    python3 tools/bootstrap/delta/exhaustion_triage.py --timed-out
    python3 tools/bootstrap/delta/exhaustion_triage.py --self-check
"""

import argparse
import dataclasses
import struct
import sys

DCOUT_MAGIC = b"\xffDCOUT\x01\x00"
DCOUT_FRAME_BYTES = 40

HALT_TAGS = {1: "reject", 2: "incomplete", 3: "internal-failure"}
COORDINATE_SPACES = {
    0: "none",
    1: "delta-source",
    2: "emitted-payload",
    3: "internal-row",
    4: "dcreq-request",
    5: "bound-support-section",
}

# The generated-program status block. Statuses 0..3 are reserved by the
# compiler-edge halt contract; 255 is deliberately unassigned so a shell's -1
# projection cannot imitate an admitted internal failure.
EVALUATOR_STATUSES = {
    132: ("alpha-vm", "illegal-instruction-trap"),
    248: ("gamma-evaluator", "internal-failure"),
    249: ("generated-program", "authored-trap"),
    250: ("gamma-evaluator", "stack-exhausted"),
    251: ("gamma-evaluator", "memory-containment-violation"),
    252: ("gamma-evaluator", "heap-exhausted"),
    253: ("gamma-evaluator", "input-extent"),
    254: ("gamma-evaluator", "output-extent"),
}


@dataclasses.dataclass(frozen=True)
class Attribution:
    owner: str
    contract: str
    detail: str

    def render(self):
        return (
            f"owner={self.owner} contract={self.contract} {self.detail}"
        ).rstrip()


def _frame_fields(output):
    """Parse a canonical 40-byte DCOUT frame, or return None."""
    if len(output) != DCOUT_FRAME_BYTES or not output.startswith(DCOUT_MAGIC):
        return None
    tag, space, _reserved, code, coordinate, limit, requested = struct.unpack(
        "<BBHIQQQ", output[8:]
    )
    return tag, space, code, coordinate, limit, requested


def classify(status, output, error, timed_out=False):
    """Attribute one observation.

    status is the process exit status (None when the run never produced one,
    e.g. a watchdog kill). output/error are the captured stdout/stderr bytes.
    """
    if timed_out or status is None:
        return Attribution(
            "host-watchdog",
            "host-timeout",
            "no compiler resource judgment is derivable from a kill",
        )
    if output.startswith(b"\xff") and _frame_fields(output) is None:
        return Attribution(
            "noncanonical",
            "malformed-frame",
            f"{len(output)} stdout bytes begin with 0xff but are not a "
            "canonical 40-byte DCOUT frame",
        )
    fields = _frame_fields(output)
    if fields is not None:
        tag, space, code, coordinate, limit, requested = fields
        if tag != status:
            return Attribution(
                "noncanonical",
                "mismatched-frame",
                f"halt status {status} disagrees with frame tag {tag}",
            )
        if tag == 0:
            return Attribution(
                "noncanonical",
                "success-frame",
                "halt 0 publishes the receipt, not a DCOUT frame",
            )
        if tag not in HALT_TAGS or space not in COORDINATE_SPACES:
            return Attribution(
                "noncanonical",
                "unknown-frame-row",
                f"tag {tag} space {space} is outside the closed tables",
            )
        stderr_note = "" if not error else " stderr-nonempty"
        return Attribution(
            "delta-compiler",
            f"dcout-{HALT_TAGS[tag]}",
            f"code={code} space={COORDINATE_SPACES[space]} "
            f"coordinate={coordinate} limit={limit} "
            f"requested={requested}{stderr_note}",
        )
    if status == 0:
        return Attribution(
            "producer",
            "complete",
            f"{len(output)} stdout bytes; not an exhaustion observation",
        )
    if status in EVALUATOR_STATUSES:
        owner, contract = EVALUATOR_STATUSES[status]
        return Attribution(
            owner,
            contract,
            "raw evaluator status; never a compiler DCOUT case"
            + ("" if not error else " stderr-nonempty"),
        )
    if status == 3 and not output:
        return Attribution(
            "gamma-evaluator",
            "function-census-preflight",
            "empty stdout status 3 is the evaluator row provision, not DCOUT",
        )
    if status == 1 and not output:
        return Attribution(
            "gamma-evaluator",
            "invalid-source",
            "status 1 with empty stdout rejects before application execution",
        )
    return Attribution(
        "noncanonical",
        "unassigned-observation",
        f"status {status} with {len(output)} stdout bytes matches no "
        "published contract",
    )


def _self_check():
    def dcout(tag, space, code, coordinate=0, limit=0, requested=0):
        return struct.pack(
            "<8sBBHIQQQ", DCOUT_MAGIC, tag, space, 0, code,
            coordinate, limit, requested,
        )

    cases = [
        # The pinned payload refusal: Incomplete code 12 at the extent.
        (dict(status=2, output=dcout(2, 2, 12, 16777212, 16777212, 477934574),
              error=b""),
         ("delta-compiler", "dcout-incomplete")),
        # Function-row refusal: source space, code 4.
        (dict(status=2, output=dcout(2, 1, 4, 720928, 32768, 32769), error=b""),
         ("delta-compiler", "dcout-incomplete")),
        # Duplicate-name rejection at the same coordinate.
        (dict(status=1, output=dcout(1, 1, 8, 720928), error=b""),
         ("delta-compiler", "dcout-reject")),
        # InternalFailure emission-metadata contradiction.
        (dict(status=3, output=dcout(3, 3, 2), error=b""),
         ("delta-compiler", "dcout-internal-failure")),
        # The witnessed b35db7dc1f long-name heap exhaustion.
        (dict(status=252, output=b"", error=b""),
         ("gamma-evaluator", "heap-exhausted")),
        (dict(status=250, output=b"", error=b""),
         ("gamma-evaluator", "stack-exhausted")),
        (dict(status=253, output=b"", error=b""),
         ("gamma-evaluator", "input-extent")),
        (dict(status=249, output=b"", error=b""),
         ("generated-program", "authored-trap")),
        (dict(status=132, output=b"", error=b""),
         ("alpha-vm", "illegal-instruction-trap")),
        # Evaluator census preflight on a generated program.
        (dict(status=3, output=b"", error=b""),
         ("gamma-evaluator", "function-census-preflight")),
        (dict(status=1, output=b"", error=b""),
         ("gamma-evaluator", "invalid-source")),
        # A successful compile publishes receipt bytes.
        (dict(status=0, output=b"\x00" * 147840, error=b""),
         ("producer", "complete")),
        # The host watchdog is not a compiler observation.
        (dict(status=None, output=b"", error=b"", timed_out=True),
         ("host-watchdog", "host-timeout")),
        # Noncanonical observations stay unattributed to either owner.
        (dict(status=2, output=dcout(1, 1, 8), error=b""),
         ("noncanonical", "mismatched-frame")),
        (dict(status=2, output=b"\xffDCOUT\x01\x00" + b"\x00" * 4,
              error=b""),
         ("noncanonical", "malformed-frame")),
        (dict(status=255, output=b"", error=b""),
         ("noncanonical", "unassigned-observation")),
        (dict(status=2, output=b"", error=b""),
         ("noncanonical", "unassigned-observation")),
    ]
    failures = 0
    for index, (observation, expected) in enumerate(cases):
        result = classify(**observation)
        got = (result.owner, result.contract)
        if got != expected:
            failures += 1
            print(f"self-check case {index}: expected {expected}, "
                  f"got {result.render()}")
    if failures:
        print(f"exhaustion triage self-check: {failures} disagreements")
        return 1
    print(f"exhaustion triage self-check: {len(cases)} classifications agree")
    return 0


def main():
    parser = argparse.ArgumentParser(
        description="Attribute a witnessed Delta compile/run observation "
        "to its allocation owner."
    )
    parser.add_argument("--status", type=int,
                        help="process exit status of the observed run")
    parser.add_argument("--stdout-file", type=argparse.FileType("rb"),
                        help="captured stdout bytes")
    parser.add_argument("--stderr-file", type=argparse.FileType("rb"),
                        help="captured stderr bytes")
    parser.add_argument("--stdout-hex",
                        help="captured stdout bytes as hex")
    parser.add_argument("--timed-out", action="store_true",
                        help="the run was killed by the host watchdog")
    parser.add_argument("--self-check", action="store_true",
                        help="classify the fixed observation table and exit")
    arguments = parser.parse_args()

    if arguments.self_check:
        return _self_check()

    output = b""
    if arguments.stdout_file is not None:
        output = arguments.stdout_file.read()
    elif arguments.stdout_hex is not None:
        output = bytes.fromhex(arguments.stdout_hex)
    error = b""
    if arguments.stderr_file is not None:
        error = arguments.stderr_file.read()

    if arguments.status is None and not arguments.timed_out:
        parser.error("supply --status, --timed-out, or --self-check")
    print(classify(
        arguments.status, output, error, timed_out=arguments.timed_out
    ).render())
    return 0


if __name__ == "__main__":
    sys.exit(main())
