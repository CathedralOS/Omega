"""Cost the existing literal successor recipe with global physical DAG sharing.

This is host-only diagnostic construction, not a Beta encoder or proof search.
It neither manufactures a theory nor claims to prove source length/traversal.
"""

import hashlib
import json
import os
import struct
import sys
from pathlib import Path

from counters import successor_rows
from lexical import certificate, checked, envelope, proposition, record, rejected


# Literal example with an operand, comment, exact address assertion, and EOF.
# Counting its bytes does not parse it or prove its encoding.
EXAMPLE = b"imm r0 0x1 ; x\n0xa:\nret\n"


def fields(encoded):
    values = struct.unpack("<" + "I" * (len(encoded) // 4), encoded)
    assert values[0] == len(values) - 1
    return values[1:]


def row_work(row, terms):
    rule, left, right, *payload = row
    if rule == 1:
        return 3
    if rule == 3:
        return 7
    if rule == 4:
        return 1 + 5 * payload[0]
    assert rule == 5
    function = terms[left - 1][1]
    if function == 36:
        return 46
    if 28 <= function <= 35:
        return 63 if function == 28 else 80
    if function in (25, 26):
        return payload[0] + 5
    assert function == 27
    return payload[0] + 7


def shared_successors(count):
    terms, proofs = [], []
    term_index, proof_index = {}, {}
    unshared_terms = unshared_proofs = unshared_work = work = 0
    for value in range(count):
        source = tuple(value.to_bytes(8, "little"))
        target = tuple((value + 1).to_bytes(8, "little"))
        carries = next(index for index, byte in enumerate(source) if byte != 255)
        rows, left, right, required, _ = successor_rows(source, target, carries)
        unshared_terms += len(rows.terms)
        unshared_proofs += len(rows.proofs)
        unshared_work += required - 5  # one combined index/root below
        term_map = {}
        for identity, encoded in enumerate(rows.terms, 1):
            tag, symbol, arity, *children = fields(encoded)
            assert len(children) == arity
            row = (tag, symbol, arity, *(term_map[child] for child in children))
            if row not in term_index:
                terms.append(row)
                term_index[row] = len(terms)
            term_map[identity] = term_index[row]
        proof_map = {}
        for identity, encoded in enumerate(rows.proofs, 1):
            rule, first, second, *payload = fields(encoded)
            if rule == 3:
                payload = [proof_map[premise] for premise in payload]
            elif rule == 4:
                payload = [payload[0], *(proof_map[premise] for premise in payload[1:])]
            else:
                assert rule in (1, 5)
            row = (rule, term_map[first], term_map[second], *payload)
            if row not in proof_index:
                proofs.append(row)
                proof_index[row] = len(proofs)
                work += row_work(row, terms)
            proof_map[identity] = proof_index[row]
    assert count > 0 and proofs[-1][1:3] == (term_map[left], term_map[right])
    metrics = dict(increments=count, ground_terms=len(terms), proof_rows=len(proofs),
                   checking_work=work + len(proofs) + 5,
                   unshared_terms=unshared_terms, unshared_proofs=unshared_proofs,
                   unshared_work=unshared_work + 5)
    owner = proposition([record(*row) for row in terms], term_map[left], term_map[right])
    witness = certificate(proofs=[record(*row) for row in proofs])
    return owner, witness, metrics


def request(definitions, count):
    owner, witness, metrics = shared_successors(count)
    framed = envelope((definitions, owner, witness))
    metrics["request_bytes"] = len(framed)
    return framed, checked(metrics["proof_rows"], metrics["checking_work"]), metrics


def main():
    from gate import invoke, prepare, require

    if len(sys.argv) != 2:
        raise SystemExit("usage: counter_cost.py PREPARED_DIRECTORY")
    evaluator, _, checker, definitions = prepare(Path(sys.argv[1]).resolve())
    # Confirm sharing and the independent work ledger, including 255 -> 256.
    for count in (1, len(EXAMPLE), 257):
        framed, expected, metrics = request(definitions, count)
        result, elapsed = invoke(evaluator, f"shared_counters_{count}", checker, framed, timeout=300)
        require(f"shared_counters_{count}", result, 0, expected)
        print(f"counter cost checked {json.dumps(metrics, sort_keys=True)} seconds={elapsed:.3f}", flush=True)
    # Corrupt only the last Transitivity row's right endpoint after valid rows.
    corrupted = framed[:-12] + framed[-16:-12] + framed[-8:]
    result, elapsed = invoke(evaluator, "shared_counters_wrong_endpoint", checker, corrupted, timeout=300)
    require("shared_counters_wrong_endpoint", result, 0, rejected(len(framed) - 12))
    print(f"counter cost: wrong endpoint rejected at {len(framed) - 12}, seconds={elapsed:.3f}", flush=True)
    source = Path(os.environ["OMEGA_PATH_GAMMA_EVALUATOR_SOURCE"]).read_bytes()
    print(f"counter cost source: bytes={len(source)} sha256={hashlib.sha256(source).hexdigest()}", flush=True)
    _, _, metrics = request(definitions, len(source))
    print(f"counter cost constructed-only {json.dumps(metrics, sort_keys=True)}", flush=True)
    print("No full-size checker invocation: this is a recipe cost, not a source-length or encoding proof.", flush=True)


if __name__ == "__main__":
    main()
