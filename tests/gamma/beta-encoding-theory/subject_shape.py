"""Full-subject storage/comparison probe, NOT a Beta encoding certificate.

Host construction supplies diagnostic lists and literal reflexivity rows only.
The Gamma checker validates them under the unchanged source-emitted theory.
No host parsing of Beta, theory generation, or artifact admission occurs.
"""

import hashlib
import os
import struct
import sys
from pathlib import Path

from gate import invoke, prepare, require
from lexical import certificate, envelope, proposition, record, rejected, checked


def subject_request(definitions, source, tape, corrupt=False):
    owners = [record(1, byte + 1, 0) for byte in range(256)]
    owners.append(record(1, 278, 0))  # shared ByteList Nil

    def byte_list(rows, raw, offset=0):
        tail = 257
        for byte in reversed(raw):
            rows.append(record(1, 279, 2, byte + 1, tail))
            tail = offset + len(rows)
        return tail

    source_root = byte_list(owners, source)
    tape_root = byte_list(owners, tape)
    witnesses = []
    source_copy = bytes([source[0] ^ 1]) + source[1:] if corrupt else source
    source_witness = byte_list(witnesses, source_copy, len(owners))
    tape_witness = byte_list(witnesses, tape, len(owners))
    owner = proposition(owners, tape_root, tape_root)
    proof_rows = (record(1, source_root, source_witness),
                  record(1, tape_root, tape_witness))
    witness = certificate(witnesses, proof_rows)
    request = envelope((definitions, owner, witness))
    # Each Cons costs four visit/resume transitions, shared Nil costs two.
    # Two rows: index3 + row checks2 + comparisons(4N+4) + final root4.
    expected = checked(2, 4 * (len(source) + len(tape)) + 13)
    if corrupt:
        first_right = 24 + len(definitions) + len(owner) + 12
        first_right += sum(map(len, witnesses)) + 12
        expected = rejected(first_right)
    return request, expected, len(owners), len(witnesses)


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: subject_shape.py PREPARED_DIRECTORY")
    temporary = Path(sys.argv[1]).resolve()
    evaluator, _, checker, definitions = prepare(temporary)
    subjects = []
    for role in ("OMEGA_PATH_GAMMA_EVALUATOR_SOURCE", "OMEGA_PATH_GAMMA_EVALUATOR_TAPE"):
        path = Path(os.environ[role])
        raw = path.read_bytes()
        subjects.append(raw)
        print(f"subject shape: {path.name} bytes={len(raw)} sha256={hashlib.sha256(raw).hexdigest()}", flush=True)
    for corrupt in (False, True):
        name = "changed_source_head" if corrupt else "complete_separate_spines"
        request, expected, owners, witnesses = subject_request(definitions, *subjects, corrupt)
        result, elapsed = invoke(evaluator, name, checker, request, timeout=300)
        require(name, result, 0, expected)
        fields = struct.unpack("<" + "Q" * ((len(expected) - 1) // 8), expected[1:])
        print(f"subject shape {name}: request={len(request)} owner_terms={owners} "
              f"witness_terms={witnesses} tag={expected[0]} fields={fields} {elapsed:.3f}s", flush=True)
    print("subject shape: complete raw-byte list comparison only; no encoding proof or artifact admission", flush=True)


if __name__ == "__main__":
    main()
