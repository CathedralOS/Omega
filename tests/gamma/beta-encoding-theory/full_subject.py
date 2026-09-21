"""Full-subject derivation production and retained-role census.

Produces the complete untrusted derivation for the owner proposition

    encode_Beta(S, 0x4000000, 0xfffffc) = Success(T)

where S is the selected evaluator's raw Beta source as a midpoint-split
Source tree and T is its persisted Alpha tape as a ByteList, verifies the
certificate against the pinned measurements below, and reports which proof
rules, theory functions, and constructors the emitted certificate actually
uses.

This driver is the checked-in generic Stepper plus an explicit gate: it
reads no evaluator output and decides no admission.  The theory section is
the sha256-pinned host-side reconstruction in identity.py, pinned against
theory.tsv exactly as gate.py pins the emitter's stdout, so the derivation
steps the same wire bytes the source-owned emitter produces.  On hosts
where the evaluator runs, prepare() supplies the identical section.

This remains diagnostic production: the certificate still has to be
produced through the selected chain and checked under the exact profile.
What this mode adds is a reproducible, pinned full-subject run plus the
usage census the retained-rule audit needs.

Requires OMEGA_PATH_GAMMA_EVALUATOR_SOURCE and OMEGA_PATH_GAMMA_EVALUATOR_TAPE
(source tools/bootstrap/paths.sh).  No evaluator materialization is needed;
the mode runs on any host with python3:

    tests/gamma/beta-encoding-theory/run.sh --full-subject
"""

import hashlib
import os
import struct
import sys
import threading
import time
from pathlib import Path

GATE_DIR = Path(__file__).resolve().parent
sys.path.append(str(GATE_DIR))
sys.path.append(str(GATE_DIR.parent / "derivation-layout"))

import identity  # noqa: E402
from lexical import certificate, envelope, proposition, record  # noqa: E402
from stepper import Stepper, Theory  # noqa: E402
from wire import MAGIC  # noqa: E402

# Subject identities pinned by tools/bootstrap/gamma/evaluator_env.sh.
SOURCE_IDENTITY = (47_756,
                   "253b42b447fbe1bae28058691d23f794573759fcd3b1ba000d250ef58ac97613")
TAPE_IDENTITY = (8_575,
                 "00c05bedbe0eed665bc165a9165ecf09cd40627bc636034ccb8dbfb24df3919d")

SOURCE_LIMIT = 0x4000000
OUTPUT_LIMIT = 0xFFFFFC
ENCODE_FUNCTION = 107
S_EMPTY, S_LEAF, S_JOIN = 285, 286, 287
R_SUCCESS = 361

RULE_NAMES = {1: "reflexivity", 2: "symmetry", 3: "transitivity",
              4: "congruence", 5: "unfolding"}

# This gate's measured certificate under the documented floor-half midpoint
# partition.  PROFILE.md records the first production's figures (owner
# 22,339/534,208, request 135,451,492, rows 3,182,484, depth 204); that run's
# ad-hoc partition detail was not retained, so this gate's table differs by
# ~0.03% while deriving the same equation.  The pins below belong to this
# construction: they fail loudly on drift, not on reproduction of the older
# run.
RECORDED = {
    "owner_bytes": 532_144,
    "owner_terms": 22_253,
    "witness_terms": 2_130_844,
    "certificate_bytes": 134_835_960,
    "proof_rows": 3_182_974,
    "reflexivity": 69_828,
    "symmetry": 0,
    "transitivity": 1_157_751,
    "congruence": 936_662,
    "unfolding": 1_018_733,
    "max_premise_depth": 204,
    "request_bytes": 135_485_028,
    "request_sha256":
        "7c0e3bf230a2675a170ea77dc6962ef6aa7c03ce15248b27475c7c6a3e592908",
}


def wire_table(data, offset):
    """Decode a u32-count-prefixed record table; return rows and end offset."""
    (count,) = struct.unpack_from("<I", data, offset)
    offset += 4
    rows = []
    for _ in range(count):
        (length,) = struct.unpack_from("<I", data, offset)
        offset += 4
        rows.append(struct.unpack_from(f"<{length}I", data, offset))
        offset += 4 * length
    return rows, offset


def check_wire(request, owner_rows, left_ref, right_ref,
               witness_rows, proof_rows):
    """Re-parse the emitted request from the wire, independently of the
    encoder helpers: framing, section magics, and every term/proof row must
    decode to exactly the rows the stepper produced."""
    if request[:8] != MAGIC:
        raise SystemExit("full subject: request magic mismatch")
    lengths = struct.unpack_from("<4I", request, 8)
    if lengths[3] != 0:
        raise SystemExit("full subject: nonzero envelope trailer")
    offset = 8 + 4 * len(lengths)
    sections = []
    for length in lengths[:3]:
        sections.append(request[offset:offset + length])
        if len(sections[-1]) != length:
            raise SystemExit("full subject: truncated request section")
        offset += length
    if offset != len(request):
        raise SystemExit("full subject: request has trailing bytes")
    _, owner_section, certificate_section = sections
    if owner_section[:4] != b"GPR1":
        raise SystemExit("full subject: proposition magic mismatch")
    owners, cursor = wire_table(owner_section, 4)
    if owners != owner_rows:
        raise SystemExit("full subject: owner table drift")
    roots = struct.unpack_from("<2I", owner_section, cursor)
    if roots != (left_ref, right_ref) or cursor + 8 != len(owner_section):
        raise SystemExit("full subject: proposition roots/length mismatch")
    if certificate_section[:4] != b"GCE1":
        raise SystemExit("full subject: certificate magic mismatch")
    witnesses, cursor = wire_table(certificate_section, 4)
    if witnesses != witness_rows:
        raise SystemExit("full subject: witness table drift")
    proofs, cursor = wire_table(certificate_section, cursor)
    if proofs != proof_rows or cursor != len(certificate_section):
        raise SystemExit("full subject: proof table drift")


def bound_subject(env, expected):
    raw = Path(os.environ[env]).read_bytes()
    actual = (len(raw), hashlib.sha256(raw).hexdigest())
    if actual != expected:
        raise SystemExit(
            f"full subject: {env} identity changed to {actual}, expected {expected}")
    return raw


def source_tree(stepper, data):
    """Midpoint-split Source tree; identical subtrees share one interned node."""
    def build(lo, hi):
        if hi - lo <= 1:
            return stepper.intern(1, S_LEAF, (stepper.byte(data[lo]),))
        mid = lo + (hi - lo) // 2
        return stepper.intern(1, S_JOIN, (build(lo, mid), build(mid, hi)))
    if not data:
        return stepper.intern(1, S_EMPTY)
    return build(0, len(data))


def premise_depths(proofs):
    """Per-row premise-chain depth; a premise-free row counts depth one."""
    depths = []
    for row in proofs:
        rule = row[0]
        if rule == 2:
            premises = row[3:4]
        elif rule == 3:
            premises = row[3:5]
        elif rule == 4:
            premises = row[4:4 + row[3]]
        else:
            premises = ()
        depth = 1
        for premise in premises:
            if depths[premise - 1] >= depth:
                depth = depths[premise - 1] + 1
        depths.append(depth)
    return depths


def census(stepper, proofs, theory):
    """Rule histogram plus per-function clause and constructor usage."""
    rules = dict.fromkeys(RULE_NAMES, 0)
    clause_use = {}
    for row in proofs:
        rules[row[0]] += 1
        if row[0] == 5:
            tag, symbol, _ = stepper.nodes[row[1] - 1]
            assert tag == 2
            clause_use.setdefault(symbol, set()).add(row[3])
    constructors = {symbol for tag, symbol, _ in stepper.nodes if tag == 1}
    functions_used = sorted(clause_use)
    clause_summary = []
    for symbol in functions_used:
        declared = len(theory.function(symbol)[4])
        clause_summary.append((symbol, len(clause_use[symbol]), declared))
    unused_functions = [i for i in range(1, len(theory.functions) + 1)
                        if i not in clause_use]
    unused_constructors = [i for i in range(1, len(theory.constructors) + 1)
                           if i not in constructors]
    return rules, clause_summary, unused_functions, unused_constructors


def build():
    """Produce the complete derivation and assembled request sections.

    Returns every artifact the certificate consumers need: the decoded
    owner/witness/proof rows, the encoded sections, the whole request, and
    the Stepper/Theory for term lookup.  Keeps the pinned identity checks
    and prints of the production gate; adds no admission decision.
    """
    started = time.monotonic()

    expected_theory = identity.fixed_identity()
    theory_bytes = identity.complete_theory()
    actual_theory = (len(theory_bytes),
                     hashlib.sha256(theory_bytes).hexdigest())
    if actual_theory != expected_theory:
        raise SystemExit(f"full subject: theory identity {actual_theory}")
    print(f"full subject: theory bytes={actual_theory[0]} "
          f"sha256={actual_theory[1]}", flush=True)

    source = bound_subject("OMEGA_PATH_GAMMA_EVALUATOR_SOURCE", SOURCE_IDENTITY)
    tape = bound_subject("OMEGA_PATH_GAMMA_EVALUATOR_TAPE", TAPE_IDENTITY)
    print(f"full subject: source={SOURCE_IDENTITY[0]}B tape={TAPE_IDENTITY[0]}B "
          "identities pinned", flush=True)

    theory = Theory(theory_bytes)
    stepper = Stepper(theory)
    subject = source_tree(stepper, source)
    result_tape = stepper.byte_list(tape)
    left = stepper.intern(2, ENCODE_FUNCTION,
                          (subject, stepper.word(SOURCE_LIMIT),
                           stepper.word(OUTPUT_LIMIT)))
    right = stepper.intern(1, R_SUCCESS, (result_tape,))
    value, _ = stepper.prove(left)
    if value != right:
        raise SystemExit(
            "full subject: derivation concluded a different right root")
    print("full subject: derivation concludes the reconstructed root "
          f"({time.monotonic() - started:.3f}s)", flush=True)

    owners, left_ref, right_ref, witnesses, proofs = stepper.encode(left, right)
    owner_records = [record(*row) for row in owners]
    witness_records = [record(*row) for row in witnesses]
    proof_records = [record(*row) for row in proofs]
    owner_section = proposition(owner_records, left_ref, right_ref)
    certificate_section = certificate(witness_records, proof_records)
    request = envelope((theory_bytes, owner_section, certificate_section))
    request_digest = hashlib.sha256(request).hexdigest()
    check_wire(request, owners, left_ref, right_ref, witnesses, proofs)

    return {
        "theory_bytes": theory_bytes,
        "theory": theory,
        "stepper": stepper,
        "owners": owners,
        "left_ref": left_ref,
        "right_ref": right_ref,
        "witnesses": witnesses,
        "proofs": proofs,
        "owner_section": owner_section,
        "certificate_section": certificate_section,
        "request": request,
        "request_digest": request_digest,
        "seconds": time.monotonic() - started,
    }


def produce():
    ctx = build()
    stepper = ctx["stepper"]
    theory = ctx["theory"]
    owners = ctx["owners"]
    witnesses = ctx["witnesses"]
    proofs = ctx["proofs"]
    owner_section = ctx["owner_section"]
    certificate_section = ctx["certificate_section"]
    request = ctx["request"]
    request_digest = ctx["request_digest"]

    rule_counts, clause_summary, unused_functions, unused_ctors = (
        census(stepper, proofs, theory))
    depths = premise_depths(stepper.proofs)

    measured = {
        "request_sha256": request_digest,
        "owner_bytes": len(owner_section),
        "owner_terms": len(owners),
        "witness_terms": len(witnesses),
        "certificate_bytes": len(certificate_section),
        "proof_rows": len(proofs),
        "max_premise_depth": max(depths),
        "request_bytes": len(request),
    }
    for tag, name in RULE_NAMES.items():
        measured[name] = rule_counts[tag]
    for key, expected in RECORDED.items():
        if measured[key] != expected:
            raise SystemExit(
                f"full subject: {key}={measured[key]}, recorded {expected}")

    elapsed = ctx["seconds"]
    print(f"full subject: owner_terms={measured['owner_terms']} "
          f"owner_bytes={measured['owner_bytes']} "
          f"witness_terms={measured['witness_terms']} "
          f"certificate_bytes={measured['certificate_bytes']} "
          f"proof_rows={measured['proof_rows']}", flush=True)
    print(f"full subject: rules "
          + " ".join(f"{name}={measured[name]}" for name in
                     ("reflexivity", "symmetry", "transitivity",
                      "congruence", "unfolding"))
          + f" max_premise_depth={measured['max_premise_depth']}", flush=True)
    print(f"full subject: request_bytes={measured['request_bytes']} "
          f"sha256={request_digest} seconds={elapsed:.3f}", flush=True)

    print(f"full subject census: {len(clause_summary)} of "
          f"{len(theory.functions)} functions unfolded; "
          f"never unfolded: {unused_functions}", flush=True)
    covered = sum(len_[1] for len_ in clause_summary)
    declared = sum(len(theory.function(symbol)[4])
                   for symbol in range(1, len(theory.functions) + 1))
    print(f"full subject census: {covered} of {declared} declared clauses "
          "appear as unfolding premises", flush=True)
    for symbol, used, total in clause_summary:
        marker = "" if used == total else f" ({used} of {total} clauses)"
        print(f"  function {symbol}: {used}/{total} clauses{marker}",
              flush=True)
    print(f"full subject census: {len(theory.constructors) - len(unused_ctors)} "
          f"of {len(theory.constructors)} constructors appear in terms; "
          f"unused: {unused_ctors}", flush=True)
    print("full subject: diagnostic production only; the certificate still "
          "has to be produced through the selected chain and checked under "
          "the exact profile", flush=True)


def main():
    # evaluate()/prove() recursion follows the derivation's premise depth;
    # run on a thread with an explicitly large stack like any deep fold.
    failure = []

    def run():
        try:
            produce()
        except BaseException as error:
            failure.append(error)

    try:
        threading.stack_size(512 * 1024 * 1024)
    except (ValueError, RuntimeError):
        pass
    worker = threading.Thread(target=run)
    worker.start()
    worker.join()
    if failure:
        raise failure[0]


if __name__ == "__main__":
    sys.setrecursionlimit(1_000_000)
    main()
