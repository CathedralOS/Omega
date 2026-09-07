"""Mode-zero and mode-one binding semantics without evaluating definitions."""

from substitution_wire import (IDENTITY, NAT, ZERO, clause, compared, envelope, example,
                               failure, function, ordinary, record, theory, vector)


def cases():
    yield vector("mode_zero_identity", compared(1, 7))
    sections = example()
    yield "format_stated_unfolding", "root", envelope(sections), compared(1, 7), 2, 60
    yield vector("distinct_equal_actual_argument", compared(1, 7),
                 (ZERO, ZERO, record(2, 1, 1, 1)), left=3, right=2)
    yield vector("wrong_actual_argument_value", compared(0, 5),
                 (ZERO, record(1, 2, 1, 1), record(2, 1, 1, 2)), left=3)
    yield vector("constant_template", compared(1, 5), (ZERO, record(2, 1, 0)),
                 definitions=theory(NAT, (ordinary((ZERO,)),)))
    yield vector("equal_numeric_ids_across_spaces_not_shortcut", compared(0, 4),
                 (record(1, 3, 0), record(2, 1, 0)),
                 definitions=theory(NAT + (record(1, 0),), (ordinary((ZERO,)),)))
    rows = (ZERO, record(1, 2, 1, 1))
    yield vector("nested_constructor_template", compared(1, 8),
                 (ZERO, record(1, 2, 1, 1), record(2, 1, 0)), left=3, right=2,
                 definitions=theory(NAT, (ordinary(rows, body=2),)))
    # Defined application template is compared as syntax, not evaluated.
    caller = ordinary((ZERO, record(2, 1, 1, 1)), body=2)
    definitions = theory(NAT, (IDENTITY, caller))
    yield vector("defined_template_application", compared(1, 8),
                 (ZERO, record(2, 1, 1, 1), record(2, 2, 0)), left=3, right=2,
                 definitions=definitions)
    yield vector("defined_template_not_normalized", compared(0, 5),
                 (ZERO, record(2, 2, 0)), definitions=definitions)
    constructors = NAT + (record(1, 0), record(1, 2, 1, 1))
    pair = ordinary((record(0, 0), record(0, 1), record(1, 4, 2, 1, 2)), (1, 1), 3)
    rows = (ZERO, record(1, 3, 0), record(1, 4, 2, 1, 2), record(2, 1, 2, 1, 2))
    yield vector("ordered_parameter_substitution", compared(1, 15), rows, left=4, right=3,
                 definitions=theory(constructors, (pair,)))
    rows = (ZERO, record(1, 3, 0), record(1, 4, 2, 2, 1), record(2, 1, 2, 1, 2))
    yield vector("wrong_substituted_first_child", compared(0, 8), rows, left=4, right=3,
                 definitions=theory(constructors, (pair,)))
    rows = (ZERO, record(1, 3, 0), record(1, 4, 2, 1, 1), record(2, 1, 2, 1, 2))
    yield vector("wrong_substituted_later_child", compared(0, 12), rows, left=4, right=3,
                 definitions=theory(constructors, (pair,)))
    constructors = (record(1, 0), record(2, 0), record(1, 1, 1))
    child = function((1,), (clause((ZERO,), constructor=1, body=1),
                          clause((record(0, 1),), constructor=3, body=1)), mode=1)
    definitions = theory(constructors, (child,), sorts=2)
    rows = (ZERO, record(1, 3, 1, 1), record(2, 1, 1, 2))
    yield vector("case_ordinal_two_is_constructor_three", compared(1, 8), rows,
                 left=3, definitions=definitions, entry="case")
    yield vector("wrong_substituted_constructor_child", compared(0, 6), rows,
                 left=3, right=2, definitions=definitions, entry="case")
    yield vector("case_zero_constructor", compared(1, 5), definitions=definitions)
    yield vector("wrong_stated_case", failure(903), rows, left=3, definitions=definitions)
    rows = (ZERO, record(2, 1, 1, 1), record(2, 1, 1, 2))
    yield vector("selected_function_is_not_evaluated", failure(903), rows,
                 left=3, definitions=definitions)
    preserve = function((2, 1), (
        clause((record(0, 0),), constructor=1, body=1),
        clause((record(0, 0), record(0, 2)), constructor=3, body=1)),
        mode=1, selected=1, result=2)
    rows = (ZERO, record(1, 2, 0), record(1, 3, 1, 1), record(2, 1, 2, 2, 3))
    yield vector("other_parameter_slot_retains_binding", compared(1, 9), rows,
                 left=4, right=2, definitions=theory(constructors, (preserve,), sorts=2), entry="case")
    yield vector("proof_semantics_remain_later", compared(1, 7), proofs=(record(2, 0, 99, 999),))
    yield vector("witness_function_and_target", compared(1, 7), (ZERO,), left=1,
                 witnesses=(ZERO, record(2, 1, 1, 1)), entry="witness")
