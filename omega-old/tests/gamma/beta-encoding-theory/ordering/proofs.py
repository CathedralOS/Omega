"""Fixed closed nibble/byte proof recipes; no theory reader or rewrite search."""

from literal_rows import LiteralRows
from lexical import certificate, checked, envelope, proposition, record, rejected


# Rows are fixed left nibbles 0..15, columns fixed right nibbles 0..15.
NIBBLE_ORDER = (
    "ELLLLLLLLLLLLLLL", "GELLLLLLLLLLLLLL", "GGELLLLLLLLLLLLL", "GGGELLLLLLLLLLLL",
    "GGGGELLLLLLLLLLL", "GGGGGELLLLLLLLLL", "GGGGGGELLLLLLLLL", "GGGGGGGELLLLLLLL",
    "GGGGGGGGELLLLLLL", "GGGGGGGGGELLLLLL", "GGGGGGGGGGELLLLL", "GGGGGGGGGGGELLLL",
    "GGGGGGGGGGGGELLL", "GGGGGGGGGGGGGELL", "GGGGGGGGGGGGGGEL", "GGGGGGGGGGGGGGGE",
)
ORDERING = {"L": 282, "E": 283, "G": 284}
CHOICE_WORK = {"L": 6, "E": 9, "G": 8}
CHOICE_CLAUSE = {"L": 1, "E": 2, "G": 3}


def diagnostic(definitions, rows, left, right, work):
    owner = proposition(rows.terms, left, right)
    return envelope((definitions, owner, certificate(proofs=rows.proofs))), checked(len(rows.proofs), work + len(rows.proofs) + 5)


def mutation(definitions, rows, left, right, index, replacement, field, code=12):
    changed = list(rows.proofs)
    changed[index - 1] = replacement
    owner = proposition(rows.terms, left, right)
    coordinate = 24 + len(definitions) + len(owner) + 12 + sum(map(len, changed[:index - 1])) + field
    return envelope((definitions, owner, certificate(proofs=changed))), rejected(coordinate, code)


def nibble_rows(rows, left, right):
    left_term = rows.term(1, 259 + left)
    right_term = rows.term(1, 259 + right)
    call = rows.term(2, 53, left_term, right_term)
    helper = rows.term(2, 37 + left, right_term)
    result = rows.term(1, ORDERING[NIBBLE_ORDER[left][right]])
    first = rows.proof(5, call, helper, left + 1)
    second = rows.proof(5, helper, result, right + 1)
    final = rows.proof(3, call, result, first, second)
    return call, result, final, left + right + 24


def split_rows(rows, left, right, function):
    # This recipe has exactly seven rows for one finite high/low comparison.
    left_nibble = left // 16 if function == 22 else left % 16
    right_nibble = right // 16 if function == 22 else right % 16
    left_split = rows.term(2, function, rows.term(1, left + 1))
    right_split = rows.term(2, function, rows.term(1, right + 1))
    first = rows.proof(5, left_split, rows.term(1, 259 + left_nibble), left + 1)
    second = rows.proof(5, right_split, rows.term(1, 259 + right_nibble), right + 1)
    explicit, result, nibble_proof, _ = nibble_rows(rows, left_nibble, right_nibble)
    call = rows.term(2, 53, left_split, right_split)
    congruence = rows.proof(4, call, explicit, 2, first, second)
    final = rows.proof(3, call, result, congruence, nibble_proof)
    work = left + right + left_nibble + right_nibble + 54
    return call, result, final, work, (first, second, congruence, explicit)


def byte_rows(rows, left, right):
    high, high_result, high_proof, high_work, high_markers = split_rows(rows, left, right, 22)
    low, low_result, low_proof, low_work, _ = split_rows(rows, left, right, 23)
    high_order = NIBBLE_ORDER[left // 16][right // 16]
    low_order = NIBBLE_ORDER[left % 16][right % 16]
    outcome = low_order if high_order == "E" else high_order
    call = rows.term(2, 55, rows.term(1, left + 1), rows.term(1, right + 1))
    choice = rows.term(2, 54, high, low)
    normalized = rows.term(2, 54, high_result, low_result)
    result = rows.term(1, ORDERING[outcome])
    unfold = rows.proof(5, call, choice, 1)
    congruence = rows.proof(4, choice, normalized, 2, high_proof, low_proof)
    selection = rows.proof(5, normalized, result, CHOICE_CLAUSE[high_order])
    normalized_proof = rows.proof(3, call, normalized, unfold, congruence)
    final = rows.proof(3, call, result, normalized_proof, selection)
    work = high_work + low_work + 38 + 11 + CHOICE_WORK[high_order] + 14
    markers = dict(choice=choice, normalized=normalized, unfold=unfold,
                   congruence=congruence, selection=selection, high_order=high_order,
                   high=high, low=low, high_proof=high_proof, low_proof=low_proof,
                   high_markers=high_markers)
    return call, result, final, work, outcome, markers
