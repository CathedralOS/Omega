"""Finite authored truth-table examples, not a Beta parser or proof producer."""

import struct
import sys
from pathlib import Path

sys.path.append(str(Path(__file__).resolve().parent.parent / "derivation-layout"))
from wire import certificate, changed_word, envelope, proposition, record


FALSE = 257
TRUE = 258
NO_HEX = 275
HEX = 276

# Each entry states one expected constructor value for each of the 256 bytes.
# These sets transcribe LANGUAGE.md's finite lexical predicates independently
# of the Gamma emitter. No emitted definitions are read to select an answer.
SOURCE_BYTES = frozenset((9, 10, 13, *range(32, 127)))
SEPARATORS = frozenset((9, 10, 13, 32, 44))
COMMENT_ENDS = frozenset((10, 13))
HEX_BYTES = b"0123456789abcdef"
BOOLEAN_TABLES = (SOURCE_BYTES, SEPARATORS, COMMENT_ENDS)


def checked(count, work):
    return b"\x07" + struct.pack("<2Q", count, work)


def rejected(coordinate, code=12):
    return b"\x01" + struct.pack("<4Q", code, coordinate, 0, 0)


def truth_table():
    terms = [record(1, byte + 1, 0) for byte in range(256)]
    false_reference = len(terms) + 1
    terms.extend((record(1, FALSE, 0), record(1, TRUE, 0)))
    no_hex_reference = len(terms) + 1
    terms.append(record(1, NO_HEX, 0))
    hex_references = []
    for nibble in range(16):
        terms.append(record(1, 259 + nibble, 0))
        terms.append(record(1, HEX, 1, len(terms)))
        hex_references.append(len(terms))
    proofs = []
    roots = []
    for function in range(1, 5):
        for byte in range(256):
            terms.append(record(2, function, 1, byte + 1))
            left = len(terms)
            if function <= 3:
                right = false_reference + int(byte in BOOLEAN_TABLES[function - 1])
            elif byte in HEX_BYTES:
                right = hex_references[HEX_BYTES.index(byte)]
            else:
                right = no_hex_reference
            proofs.append(record(5, left, right, byte + 1))
            roots.append((left, right))
    return terms, proofs, roots, false_reference, no_hex_reference, hex_references


def cases(definitions):
    terms, proofs, roots, false_reference, no_hex_reference, hex_references = truth_table()
    owner = proposition(terms, *roots[-1])
    proof_start = 24 + len(definitions) + len(owner) + 12

    def vector(name, rows, expected, theory=definitions):
        return name, envelope((theory, owner, certificate(proofs=rows))), expected

    # Setup 1025; four clause walks sum to 131584; 1024 nullary overheads
    # cost 5120; sixteen unary Hex bodies add 48; final root costs four.
    yield vector("all_1024_lexical_truths", proofs, checked(1024, 137781))

    # Corrupt a true and a false row for every predicate, plus both hex forms
    # and the nibble payload. Most errors follow an already valid proof prefix.
    wrong_answers = (
        ("source_tab_rejected", 9, false_reference),
        ("source_del_admitted", 127, false_reference + 1),
        ("separator_comma_rejected", 256 + 44, false_reference),
        ("separator_semicolon_admitted", 256 + 59, false_reference + 1),
        ("comment_cr_continues", 512 + 13, false_reference),
        ("comment_tab_ends", 512 + 9, false_reference + 1),
        ("hex_lowercase_f_rejected", 768 + 102, no_hex_reference),
        ("hex_uppercase_A_admitted", 768 + 65, hex_references[10]),
        ("hex_f_wrong_nibble", 768 + 102, hex_references[14]),
        ("late_byte_255_hex_admitted", 1023, hex_references[0]),
    )
    for name, index, wrong_right in wrong_answers:
        changed = list(proofs)
        changed[index] = record(5, roots[index][0], wrong_right, index % 256 + 1)
        yield vector(name, changed, rejected(proof_start + index * 20 + 12))

    for name, index, wrong_clause in (
        ("zero_clause", 0, 0),
        ("out_of_range_clause", 255, 257),
        ("wrong_separator_clause", 256 + 44, 44),
        ("late_wrong_hex_clause", 1023, 255),
    ):
        changed = list(proofs)
        changed[index] = record(5, *roots[index], wrong_clause)
        yield vector(name, changed, rejected(proof_start + index * 20 + 16, 10))

    # Fixed physical layout: header12 + 275 nullary constructors*12 + Hex16
    # + function_count4 + first function header28 + clause0's template symbol20.
    # Flip source_byte(0) from False to True. This theory still forms, so the
    # unchanged row must reject its equation; no package digest is a proof rule.
    symbol_offset = 12 + 275 * 12 + 16 + 4 + 28 + 20
    if definitions[symbol_offset:symbol_offset + 4] != struct.pack("<I", FALSE):
        raise SystemExit("Beta lexical theory: fixed mutation field changed")
    altered = changed_word(definitions, symbol_offset, TRUE)
    yield vector("altered_formed_theory_body", proofs, rejected(proof_start + 12), altered)

    # A completely valid table still cannot conclude a different owner root.
    wrong_owner = proposition(terms, roots[0][0], roots[0][1])
    yield (
        "valid_table_wrong_owner_root",
        envelope((definitions, wrong_owner, certificate(proofs=proofs))),
        rejected(proof_start + 1023 * 20 + 8),
    )
