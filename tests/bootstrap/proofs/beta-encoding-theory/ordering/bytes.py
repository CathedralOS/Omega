"""Literal byte examples distinguish nibble priority and unsigned ordering."""

from .proofs import CHOICE_CLAUSE, LiteralRows, ORDERING, byte_rows, diagnostic, mutation, record


BYTE_CASES = (
    ("zero_equal", 0, 0, "E"), ("maximum_equal", 255, 255, "E"),
    ("varied_equal", 165, 165, "E"), ("zero_maximum", 0, 255, "L"),
    ("maximum_zero", 255, 0, "G"), ("high_dominates_less", 31, 32, "L"),
    ("high_dominates_greater", 32, 31, "G"), ("low_fallback_less", 160, 175, "L"),
    ("low_fallback_greater", 175, 160, "G"), ("unsigned_high_bit", 128, 127, "G"),
    ("below_high_bit", 127, 128, "L"), ("adjacent_maximum", 254, 255, "L"),
)


def cases(definitions):
    for name, left, right, expected in BYTE_CASES:
        rows = LiteralRows()
        call, _, final, work, _, markers = byte_rows(rows, left, right)
        result = rows.term(1, ORDERING[expected])
        yield f"ordering_byte_{name}", *diagnostic(definitions, rows, call, result, work)
        if name not in ("high_dominates_less", "unsigned_high_bit"):
            continue
        wrong = rows.term(1, ORDERING["G" if expected == "L" else "L"])
        selection = markers["selection"]
        yield f"ordering_byte_{name}_wrong_answer", *mutation(definitions, rows, call, result, selection, record(5, markers["normalized"], wrong, CHOICE_CLAUSE[markers["high_order"]]), 12)
        yield f"ordering_byte_{name}_missing_normalization", *mutation(definitions, rows, call, result, selection, record(5, markers["choice"], result, CHOICE_CLAUSE[markers["high_order"]]), 16, 10)
        yield f"ordering_byte_{name}_swapped_premises", *mutation(definitions, rows, call, result, markers["congruence"], record(4, markers["choice"], markers["normalized"], 2, markers["low_proof"], markers["high_proof"]), 20)
        yield f"ordering_byte_{name}_missing_congruence", *mutation(definitions, rows, call, result, markers["congruence"], record(1, markers["choice"], markers["normalized"]), 12)
        # Physical owner coordinates and record lengths are fixed independently.
        yield f"ordering_byte_{name}_wrong_owner_root", *mutation(definitions, rows, call, wrong, final, rows.proofs[final - 1], 12)
