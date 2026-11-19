"""Definitions supply one checked equation, never structural memo facts."""

from proof_wire import (IDENTITY, NAT, ZERO, checked, clause, failure, function,
                        proof_row, record, theory, vector)


def cases():
    definitions = theory(NAT, (IDENTITY,))
    owners = (ZERO, record(2, 1, 1, 1))
    first = record(5, 2, 1, 1)
    start = proof_row(definitions=definitions, owners=owners)
    yield vector("unfold_left_must_be_function", failure(start + 8, 10),
                 (record(5, 1, 1, 1),), owners, definitions=definitions)
    for clause_id in (0, 2, 99):
        yield vector(f"unfold_clause_{clause_id}", failure(start + 16, 10),
                     (record(5, 2, 1, clause_id),), owners, definitions=definitions)
    yield vector("successful_unfold_does_not_admit_invalid_reflexivity",
                 failure(proof_row((first,), definitions, owners) + 12),
                 (first, record(1, 2, 1)), owners, definitions=definitions)
    different = (ZERO, record(1, 2, 1, 1), record(2, 1, 1, 2))
    yield vector("unfold_wrong_substituted_value",
                 failure(proof_row(definitions=definitions, owners=different) + 12),
                 (record(5, 3, 1, 1),), different, definitions=definitions)
    next_terms = owners + (record(1, 2, 1, 2), record(1, 2, 1, 1))
    yield vector("unfold_then_congruence", checked(2, 21),
                 (first, record(4, 3, 4, 1, 1)), next_terms, left=3, right=4, definitions=definitions)
    yield vector("congruence_right_argument_relation",
                 failure(proof_row((first,), definitions, next_terms) + 20),
                 (first, record(4, 3, 3, 1, 1)), next_terms, definitions=definitions)
    constant = function((), (clause((ZERO,), body=1),))
    constants = (ZERO, record(2, 1, 0))
    constant_theory = theory(NAT, (constant,))
    yield vector("congruence_tag_before_count",
                 failure(proof_row(definitions=constant_theory, owners=constants) + 12),
                 (record(4, 1, 2, 99, *([1] * 99)),), constants, definitions=constant_theory)
    case_function = function((1,), (
        clause((ZERO,), constructor=1, body=1),
        clause((record(0, 1),), constructor=2, body=1)), mode=1)
    case_theory = theory(NAT, (case_function,))
    case_terms = (ZERO, record(1, 2, 1, 1), record(2, 1, 1, 2))
    yield vector("case_child_unfolding", checked(1, 15), (record(5, 3, 1, 2),),
                 case_terms, left=3, definitions=case_theory)
    yield vector("wrong_case_constructor",
                 failure(proof_row(definitions=case_theory, owners=case_terms) + 16, 10),
                 (record(5, 3, 1, 1),), case_terms, definitions=case_theory)
    two_functions = theory(NAT, (IDENTITY, case_function))
    hidden_constructor = (ZERO, record(2, 1, 1, 1), record(2, 2, 1, 2))
    yield vector("prior_equation_does_not_evaluate_case_subject",
                 failure(proof_row((first,), two_functions, hidden_constructor) + 16, 10),
                 (first, record(5, 3, 1, 1)), hidden_constructor, definitions=two_functions)
