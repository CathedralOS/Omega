#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon evaluator entry: skipped (python3 absent)"
    exit 0
}

ENTRY_TMP=$(mktemp -d)
trap 'rm -rf -- "$ENTRY_TMP"' EXIT HUP INT TERM
# Bound materializers refuse before writing when the canonical manifest,
# members, packed closure, entry source, or receipt differ from the audited
# edge records.
materialize_epsilon_evaluator "$ENTRY_TMP/epsilon_compiler.delta"
require_epsilon_evaluator_entry_identity
materialize_delta_compiler "$ENTRY_TMP/delta_compiler.gamma"
materialize_delta_support "$ENTRY_TMP/support.bin"
materialize_gamma_evaluator "$ENTRY_TMP/evaluator" >/dev/null

# The canonical receipt: packed closure + canonical entry through the bound
# Delta compiler, reconstructed once per run and bound by identity below.
GATE_DIR="$GATE_DIR" ENTRY_TMP="$ENTRY_TMP" python3 - <<'PY'
import os
import signal
import struct
import subprocess
from pathlib import Path

gate = Path(os.environ["GATE_DIR"])
temporary = Path(os.environ["ENTRY_TMP"])
closure = (temporary / "epsilon_compiler.delta").read_bytes()
entry = (gate / "evaluator_entry.delta").read_bytes()
compiler = (temporary / "delta_compiler.gamma").read_bytes()
support = (temporary / "support.bin").read_bytes()

process = subprocess.Popen(
    [str(temporary / "evaluator")], stdin=subprocess.PIPE,
    stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
)
subject = closure + entry
sealed = (b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject))
          + subject + support)
try:
    receipt, error = process.communicate(
        struct.pack("<I", len(compiler)) + compiler + sealed, timeout=600)
except subprocess.TimeoutExpired:
    os.killpg(process.pid, signal.SIGKILL)
    process.wait()
    raise SystemExit("Epsilon evaluator entry: selected Gamma timed out")
if error:
    raise SystemExit(f"unexpected evaluator stderr: {error!r}")
if process.returncode != 0:
    raise SystemExit(
        f"canonical entry compilation returned {process.returncode}")
if not receipt.startswith(b"(def $application () Int 1)\n"):
    raise SystemExit("canonical receipt lost its application marker")
(temporary / "canonical.gamma").write_bytes(receipt)
PY

require_epsilon_evaluator_entry_receipt_identity "$ENTRY_TMP/canonical.gamma"

GATE_DIR="$GATE_DIR" ENTRY_TMP="$ENTRY_TMP" python3 - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path

temporary = Path(os.environ["ENTRY_TMP"])
closure = (temporary / "epsilon_compiler.delta").read_bytes()
receipt = (temporary / "canonical.gamma").read_bytes()
closure_digest = hashlib.sha256(closure).digest()

EREQ = b"EEREQ\x01\x00\x00"
EEOUT = b"\xffEEOUT\x01\x00"


def evaluate(program, sealed_input, timeout=300):
    process = subprocess.Popen(
        [str(temporary / "evaluator")], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, error = process.communicate(
            struct.pack("<I", len(program)) + program + sealed_input,
            timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("Epsilon evaluator entry: selected Gamma timed out")
    if error:
        raise SystemExit(f"unexpected evaluator stderr: {error!r}")
    return process.returncode, output


def ereq(source=b"", stdin=b"", profile=1, digest=closure_digest):
    return (EREQ + struct.pack("<III", profile, len(source), len(stdin))
            + digest + source + stdin)


def refusal(code, coordinate, limit=0, requested=0, outcome=1, space=1):
    frame = struct.pack(
        "<8sBBHIQQQ", EEOUT, outcome, space, 0, code, coordinate,
        limit, requested)
    assert len(frame) == 40
    return 0, frame


cases = []          # (name, request, expected (status, stdout))
admissions = []     # requests admitted to the evaluator -> canonical observation


def expect_observation(name, request, expected):
    admissions.append((name, request, (0, expected)))


# --- Request envelope: exact and adjacent boundaries ---------------------

# Header extent: 0..51 bytes all refuse short_header at the observed extent;
# exactly 52 is a complete header (zero sections) admitted to the evaluator.
for extent in range(52):
    cases.append((f"header truncation {extent}", ereq()[:extent],
                  refusal(1, extent)))
expect_observation("exact 52-byte zero-section request", ereq(),
                   b"\x02\x08\x00\x00\x00\x00")

# Identity and reserved bytes: each adjacent byte refuses at its own offset.
for index in range(8):
    changed = bytearray(ereq())
    changed[index] ^= 0xFF
    cases.append((f"identity byte {index}", bytes(changed),
                  refusal(2, index)))
changed = bytearray(ereq())
changed[1] ^= 0xFF
changed[7] ^= 0xFF
cases.append(("first of two incorrect identity bytes", bytes(changed),
              refusal(2, 1)))

# Profile: only 1 (ExactConsoleV1) is assigned in version 1.
for profile in (0, 2, 3, 0xFFFFFFFF):
    cases.append((f"unknown profile {profile}", ereq(profile=profile),
                  refusal(3, 8)))

# Length fields are u31: the high bit of each length refuses at its byte.
changed = bytearray(ereq())
changed[15] = 0x80
cases.append(("source length high bit", bytes(changed), refusal(4, 15)))
changed = bytearray(ereq())
changed[19] = 0x80
cases.append(("stdin length high bit", bytes(changed), refusal(4, 19)))

# Artifact binding: each adjacent digest byte refuses artifact_mismatch at
# 20 + index; the exact bound digest is admitted by every other case.
for index in range(32):
    changed = bytearray(ereq())
    changed[20 + index] ^= 0xFF
    cases.append((f"closure identity byte {index}", bytes(changed),
                  refusal(5, 20 + index)))
changed = bytearray(ereq())
changed[20] ^= 0xFF
changed[51] ^= 0xFF
cases.append(("first of two incorrect identity digest bytes",
              bytes(changed), refusal(5, 20)))

# Section extents: a declared section beyond the remaining bytes refuses
# section_extent with limit = remaining bytes and requested = declared.
with_source_one = ereq()[:12] + struct.pack("<I", 1) + ereq()[16:]
cases.append(("declared source absent", with_source_one,
              refusal(6, 12, 0, 1)))
with_source_two = ereq(b"A")[:12] + struct.pack("<I", 2) + ereq(b"A")[16:]
cases.append(("declared source one beyond remaining", with_source_two,
              refusal(6, 12, 1, 2)))
with_stdin_one = ereq()[:16] + struct.pack("<I", 1) + ereq()[20:]
cases.append(("declared stdin absent", with_stdin_one,
              refusal(6, 16, 0, 1)))
with_stdin_two = ereq(stdin=b"z")[:16] + struct.pack("<I", 2) \
    + ereq(stdin=b"z")[20:]
cases.append(("declared stdin one beyond remaining", with_stdin_two,
              refusal(6, 16, 1, 2)))
maximum_length = ereq()[:12] + struct.pack("<I", 0x7FFFFFFF) + ereq()[16:]
cases.append(("declared source at u31 maximum absent", maximum_length,
              refusal(6, 12, 0, 0x7FFFFFFF)))
expect_observation("one-byte source section exact",
                   ereq(b"\x00"), b"\x02\x01\x00\x00\x00\x00")

# Exact end: a byte after the complete envelope refuses trailing_input at
# the first trailing offset.
cases.append(("trailing byte after zero-section request", ereq() + b"x",
              refusal(7, 52)))
cases.append(("trailing byte after declared sections",
              ereq(b"\x00", b"ab") + b"x", refusal(7, 55)))

# --- Canonical observations through the admitted path --------------------

EXIT_WRITE = b"""boundary trait Console {
  machine exit_process(return_code: i32) -> never;
  machine write_byte(value: i32);
  machine read_byte() -> i32;
  machine write_line(text: &[u8]);
}

data Main {
  console: Console;
}

machine Main::main(&mut self) {
  self.console.write_byte(65);
  self.console.exit_process(42);
}
"""
expect_observation("exit with stdout", ereq(EXIT_WRITE),
                   b"\x00" + struct.pack("<i", 42) + b"A")
expect_observation("unconsumed stdin section", ereq(EXIT_WRITE, b"extra"),
                   b"\x00" + struct.pack("<i", 42) + b"A")

STDIN_ECHO = b"""boundary trait Console {
  machine exit_process(return_code: i32) -> never;
  machine write_byte(value: i32);
  machine read_byte() -> i32;
  machine write_line(text: &[u8]);
}

data Main {
  console: Console;
}

machine Main::main(&mut self) {
  let value: i32 = self.console.read_byte();
  self.console.write_byte(value);
  self.console.exit_process(value);
}
"""
expect_observation("sealed stdin reaches Console", ereq(STDIN_ECHO, b"Z"),
                   b"\x00" + struct.pack("<i", 90) + b"Z")

TRAP_WRITE = b"""boundary trait Console {
  machine exit_process(return_code: i32) -> never;
  machine write_byte(value: i32);
  machine read_byte() -> i32;
  machine write_line(text: &[u8]);
}

data Main {
  console: Console;
}

machine Main::main(&mut self) {
  self.console.write_byte(66);
  let value: i32 = 1 / 0;
}
"""
expect_observation("trap with stdout prefix", ereq(TRAP_WRITE),
                   b"\x01\x02B")

# Sparse storage through the canonical edge: a [i32; 2147483647] field cannot
# be realized densely inside the pair arena, so its exact reads and writes
# witness the evaluator's sparse immutable account on this receipt.
SPARSE_ARRAY = b"""boundary trait Console {
  machine exit_process(return_code: i32) -> never;
  machine write_byte(value: i32);
  machine read_byte() -> i32;
  machine write_line(text: &[u8]);
}

data Main {
  console: Console;
  values: [i32; 2147483647];
}

machine Main::main(&mut self) {
  self.values[1073741824] = 7;
  self.values[2147483646] = 9;
  assert self.values[1073741824] == 7;
  assert self.values[0] == 0;
  self.console.write_byte(65);
}
"""
expect_observation("large sparse array through the canonical edge",
                   ereq(SPARSE_ARRAY), b"\x00\x00\x00\x00\x00A")

# --- Incomplete transport outcomes ----------------------------------------
#
# A lower-chain refusal cannot be intercepted by the evaluator program: the
# adapter refuses an oversized sealed input before `main` runs, and evaluator
# context or pair exhaustion halts the process outright. At the edge boundary
# a nonzero status with empty stdout is the section-10 outer Incomplete; the
# status and the requester's own submitted extents carry its fields
# (bootstrap/4_epsilon/EVALUATOR_ENTRY.md).

INCOMPLETE = {
    # status: (resource, limit, requested)
    250: ("evaluator live call contexts", 256, 257),
    252: ("cumulative immutable pair nodes", 40265318, 40265319),
    253: ("sealed input", 4194304, None),  # requested = submitted extent
    254: ("published observation", 4194304, 4194305),
}
incomplete = []  # (name, request, status, resource, limit, requested)


def expect_incomplete(name, request, status, requested=None):
    resource, limit, refused = INCOMPLETE[status]
    if requested is None:
        requested = len(request)
    incomplete.append((name, request, status, resource, limit, requested))


# The adapter's 4,194,304-byte sealed-input bound is the transport boundary.
# Exactly at the bound the EREQ envelope is admitted (and then refuses its
# trailing section with EEOUT, still no observation); one byte beyond is the
# Incomplete sealed-input refusal - status 253 with empty stdout, no frame,
# no observation.
at_bound = ereq() + b" " * (4194304 - 52)
cases.append(("sealed input at exact bound", at_bound, refusal(7, 52)))
expect_incomplete("sealed input one beyond bound", at_bound + b"x", 253)

# Live call contexts are an execution-side counter on the same receipt: the
# evaluator's 256-context bound admits 34 nested non-tail machine calls and
# refuses the 35th before any observation - Incomplete via status 250. This
# exact/adjacent pair is a measured property of this artifact, not an Epsilon
# language limit.
NESTED_CALLS = b"""boundary trait Console {
  machine exit_process(return_code: i32) -> never;
  machine write_byte(value: i32);
  machine read_byte() -> i32;
  machine write_line(text: &[u8]);
}

data Main {
  console: Console;
}

machine Main::main(&mut self) {
  let total: i32 = nest(DEPTH);
  self.console.write_byte(65);
  self.console.exit_process(total);
}

machine nest(depth: i32) -> i32 {
  transition depth {
    0 -> return 0
    _ -> recurse()
  }
  state recurse() {
    let subtotal: i32 = nest(depth - 1);
    return subtotal + 1;
  }
}
"""
expect_observation(
    "34 nested calls admitted",
    ereq(NESTED_CALLS.replace(b"DEPTH", b"34")),
    b"\x00" + struct.pack("<i", 34) + b"A",
)
expect_incomplete(
    "35th nested call refused",
    ereq(NESTED_CALLS.replace(b"DEPTH", b"35")), 250, 257)

for name, request, expected in cases + admissions:
    actual = evaluate(receipt, request)
    if actual != expected:
        raise SystemExit(
            f"{name}: expected status {expected[0]} and {expected[1].hex()}, "
            f"got status {actual[0]}, {len(actual[1])} bytes, "
            f"prefix {actual[1][:80].hex()}")

for name, request, status, resource, limit, requested in incomplete:
    actual = evaluate(receipt, request)
    if actual != (status, b""):
        raise SystemExit(
            f"{name}: expected Incomplete({resource}, {limit}, {requested}) "
            f"via status {status} with empty stdout, got status "
            f"{actual[0]}, {len(actual[1])} bytes, "
            f"prefix {actual[1][:80].hex()}")

print(
    f"Epsilon evaluator entry: {len(cases)} exact/adjacent EEOUT controls, "
    f"{len(incomplete)} Incomplete transport outcomes, and "
    f"{len(admissions)} canonical observations passed"
)
PY
