"""Backward proof custody and term references, including unused rows."""

from proof_wire import ZERO, failure, proof_count, proof_row, record, theory, vector


def cases():
    yield vector("empty_proof_table", failure(proof_count()), ())
    for value in (0, 2, 99):
        yield vector(f"left_term_{value}", failure(proof_row() + 8), (record(1, value, 1),))
        yield vector(f"right_term_{value}", failure(proof_row() + 12), (record(1, 1, value),))
    yield vector("left_term_before_right", failure(proof_row() + 8), (record(1, 0, 99),))
    definitions = theory((record(1, 0), record(2, 0)), sorts=2)
    owners = (ZERO, record(1, 2, 0))
    yield vector("proof_conclusion_sort", failure(proof_row(definitions=definitions, owners=owners) + 12),
                 (record(1, 1, 2),), owners, definitions=definitions)
    first = record(1, 1, 1)
    second = proof_row((first,))
    for value in (0, 2, 3, 99):
        yield vector(f"symmetry_premise_{value}", failure(second + 16),
                     (first, record(2, 1, 1, value)))
        yield vector(f"transitivity_first_premise_{value}", failure(second + 16),
                     (first, record(3, 1, 1, value, 1)))
        yield vector(f"transitivity_second_premise_{value}", failure(second + 20),
                     (first, record(3, 1, 1, 1, value)))
    yield vector("both_transitivity_premises_before_comparisons", failure(second + 16),
                 (first, record(3, 1, 1, 0, 99)))
    yield vector("unused_future_premise_row_rejects", failure(proof_row() + 16),
                 (record(2, 1, 1, 2), first))
    yield vector("cyclic_premises_reject_first_edge", failure(proof_row() + 16),
                 (record(2, 1, 1, 2), record(2, 1, 1, 1)))
    yield vector("valid_prefix_invalid_final_term", failure(second + 8), (first, record(1, 0, 0)))
    yield vector("invalid_unused_row_before_valid_final", failure(second + 16),
                 (first, record(2, 1, 1, 0), first))
