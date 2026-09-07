"""Literal unsigned Word comparisons and a fixed eight-byte proof composition."""

from .proofs import CHOICE_CLAUSE, CHOICE_WORK, LiteralRows, ORDERING, byte_rows, diagnostic, mutation, record


ZERO = (0, 0, 0, 0, 0, 0, 0, 0)
MAXIMUM = (255, 255, 255, 255, 255, 255, 255, 255)
HIGH_BIT = (0, 0, 0, 0, 0, 0, 0, 128)
BELOW_HIGH_BIT = (255, 255, 255, 255, 255, 255, 255, 127)
VARIED = (239, 205, 171, 137, 103, 69, 35, 1)
# These are the actual provision words in Beta LANGUAGE.md, low byte first.
SOURCE_LIMIT = (0, 0, 0, 4, 0, 0, 0, 0)  # 0x4000000
OUTPUT_LIMIT = (252, 255, 255, 0, 0, 0, 0, 0)  # 0xfffffc

# Highest differing byte advances through all eight slots. Every less
# significant byte opposes that comparison; more significant bytes agree.
HIGHEST_DIFFERENCES = (
    ((16, 42, 128, 254, 19, 77, 201, 33), (17, 42, 128, 254, 19, 77, 201, 33)),
    ((255, 16, 128, 254, 19, 77, 201, 33), (0, 17, 128, 254, 19, 77, 201, 33)),
    ((255, 255, 16, 254, 19, 77, 201, 33), (0, 0, 17, 254, 19, 77, 201, 33)),
    ((255, 255, 255, 16, 19, 77, 201, 33), (0, 0, 0, 17, 19, 77, 201, 33)),
    ((255, 255, 255, 255, 16, 77, 201, 33), (0, 0, 0, 0, 17, 77, 201, 33)),
    ((255, 255, 255, 255, 255, 16, 201, 33), (0, 0, 0, 0, 0, 17, 201, 33)),
    ((255, 255, 255, 255, 255, 255, 16, 33), (0, 0, 0, 0, 0, 0, 17, 33)),
    ((255, 255, 255, 255, 255, 255, 255, 16), (0, 0, 0, 0, 0, 0, 0, 17)),
)

WORD_CASES = (
    ("zero_equal", ZERO, ZERO, "E"),
    ("maximum_equal", MAXIMUM, MAXIMUM, "E"),
    ("varied_equal", VARIED, VARIED, "E"),
    ("zero_maximum", ZERO, MAXIMUM, "L"),
    ("maximum_zero", MAXIMUM, ZERO, "G"),
    ("unsigned_high_bit", HIGH_BIT, BELOW_HIGH_BIT, "G"),
    ("below_high_bit", BELOW_HIGH_BIT, HIGH_BIT, "L"),
    ("high_bit_below_maximum", HIGH_BIT, MAXIMUM, "L"),
    *((f"highest_difference_{slot}_less", left, right, "L") for slot, (left, right) in enumerate(HIGHEST_DIFFERENCES)),
    *((f"highest_difference_{slot}_greater", right, left, "G") for slot, (left, right) in enumerate(HIGHEST_DIFFERENCES)),
    ("source_limit_below", (255, 255, 255, 3, 0, 0, 0, 0), SOURCE_LIMIT, "L"),
    ("source_limit_exact", SOURCE_LIMIT, SOURCE_LIMIT, "E"),
    ("source_limit_above", (1, 0, 0, 4, 0, 0, 0, 0), SOURCE_LIMIT, "G"),
    ("output_limit_below", (251, 255, 255, 0, 0, 0, 0, 0), OUTPUT_LIMIT, "L"),
    ("output_limit_exact", OUTPUT_LIMIT, OUTPUT_LIMIT, "E"),
    ("output_limit_above", (253, 255, 255, 0, 0, 0, 0, 0), OUTPUT_LIMIT, "G"),
)


def word_rows(left, right):
    rows = LiteralRows()
    left_bytes = [rows.term(1, value + 1) for value in left]
    right_word = rows.term(1, 277, *(rows.term(1, value + 1) for value in right))
    left_word = rows.term(1, 277, *left_bytes)
    public = rows.term(2, 57, left_word, right_word)
    helper = rows.term(2, 56, *left_bytes, right_word)
    public_unfold = rows.proof(5, public, helper, 1)
    # All eight literal byte pairs are proved, even when a higher byte differs.
    comparisons = [byte_rows(rows, first, second) for first, second in zip(left, right)]
    current, result, proof, _, outcome, _ = comparisons[0]
    work = 51 + 128 + 14 + sum(comparison[3] for comparison in comparisons)
    markers = {}
    for position in range(1, 8):
        high, high_result, high_proof, _, high_outcome, _ = comparisons[position]
        choice = rows.term(2, 54, high, current)
        normalized = rows.term(2, 54, high_result, result)
        outcome = outcome if high_outcome == "E" else high_outcome
        next_result = rows.term(1, ORDERING[outcome])
        congruence = rows.proof(4, choice, normalized, 2, high_proof, proof)
        selection = rows.proof(5, normalized, next_result, CHOICE_CLAUSE[high_outcome])
        final = rows.proof(3, choice, next_result, congruence, selection)
        work += 18 + CHOICE_WORK[high_outcome]
        markers.update(choice=choice, normalized=normalized, congruence=congruence,
                       selection=selection, high_proof=high_proof, lower_proof=proof,
                       lower_result=result, high_outcome=high_outcome)
        current, result, proof = choice, next_result, final
    helper_unfold = rows.proof(5, helper, current, 1)
    helper_proof = rows.proof(3, helper, result, helper_unfold, proof)
    final = rows.proof(3, public, result, public_unfold, helper_proof)
    markers.update(helper=helper, helper_unfold=helper_unfold, public_unfold=public_unfold)
    return rows, public, result, final, work, markers


def cases(definitions):
    for name, left, right, expected in WORD_CASES:
        rows, public, _, final, work, markers = word_rows(left, right)
        result = rows.term(1, ORDERING[expected])
        yield f"ordering_word_{name}", *diagnostic(definitions, rows, public, result, work)
        if name not in ("unsigned_high_bit", "highest_difference_7_less", "varied_equal"):
            continue
        wrong = rows.term(1, ORDERING["G" if expected == "L" else "L"])
        selection = markers["selection"]
        yield f"ordering_word_{name}_wrong_answer", *mutation(definitions, rows, public, result, selection, record(5, markers["normalized"], wrong, CHOICE_CLAUSE[markers["high_outcome"]]), 12)
        yield f"ordering_word_{name}_wrong_clause", *mutation(definitions, rows, public, result, markers["helper_unfold"], record(5, markers["helper"], markers["choice"], 2), 16, 10)
        yield f"ordering_word_{name}_missing_normalization", *mutation(definitions, rows, public, result, selection, record(5, markers["choice"], result, CHOICE_CLAUSE[markers["high_outcome"]]), 16, 10)
        yield f"ordering_word_{name}_missing_congruence", *mutation(definitions, rows, public, result, markers["congruence"], record(1, markers["choice"], markers["normalized"]), 12)
        yield f"ordering_word_{name}_swapped_premises", *mutation(definitions, rows, public, result, markers["congruence"], record(4, markers["choice"], markers["normalized"], 2, markers["lower_proof"], markers["high_proof"]), 20)
        yield f"ordering_word_{name}_wrong_owner_root", *mutation(definitions, rows, public, wrong, final, rows.proofs[final - 1], 12)
        if name == "highest_difference_7_less":
            yield "ordering_word_ignores_highest_byte", *mutation(definitions, rows, public, result, selection, record(5, markers["normalized"], markers["lower_result"], 1), 12)
