"""Conservative definitions with explicit finite inhabitants and local slots."""

from theory_wire import (NAT, ZERO, certificate, clause, envelope, example, formed,
                         function, natural_cases, ordinary, proposition, record, vector)


def cases():
    sections = example()
    yield "format_identity", envelope(sections), formed(sections, 1, 2, 1), 2, 60
    yield vector("free_nullary_seed", (record(1, 0),))
    yield vector("multiple_base_sorts", (record(1, 0), record(2, 0)), sorts=2)
    yield vector("productive_mutual_cycle", (
        record(1, 1, 2), record(2, 1, 1), record(2, 0)), sorts=2)
    yield vector("reverse_inhabitation_chain", (
        record(4, 1, 3), record(3, 1, 2), record(2, 1, 1), record(1, 0)), sorts=4)
    yield vector("zero_argument_definition", functions=(ordinary((ZERO,)),))
    identity = ordinary((record(0, 0),), (1,))
    caller = ordinary((record(0, 0), record(2, 1, 1, 1), record(1, 2, 1, 2)), (1,), 3)
    yield vector("nested_prior_helper_and_constructor", functions=(identity, caller))
    yield vector("self_call_on_immediate_child", functions=(natural_cases(
        (record(0, 1), record(2, 1, 1, 1)), 2),))
    sharing = (ZERO, record(1, 2, 1, 1), record(1, 3, 2, 2, 2), ZERO)
    yield vector("shared_and_unused_valid_rows", NAT + (record(1, 2, 1, 1),),
                 (ordinary(sharing, body=3),))
    two_sorts = (record(1, 0), record(2, 0), record(1, 2, 1, 2))
    base = clause((record(0, 0), record(0, 2), ZERO), constructor=1, body=3)
    branch = clause((record(0, 0), record(0, 2), record(0, 3), record(0, 4),
                     record(2, 1, 3, 1, 3, 2)), constructor=3, body=5)
    # Selected slot1 is absent; slots0/2 retain sort2, children3/4 have sorts1/2.
    recursive = function((2, 1, 2), (base, branch), mode=1, selected=1)
    helper = ordinary((record(0, 0), record(0, 1), record(2, 1, 3, 2, 1, 2)), (1, 2), 3)
    yield vector("multiple_children_other_parameters_and_functions", two_sorts,
                 (recursive, helper), sorts=2)
    yield vector("same_slots_are_local_to_each_function", functions=(identity, identity))
    yield vector("repeated_references_do_not_expand_terms", NAT + (record(1, 2, 1, 1),),
                 (ordinary((ZERO, record(1, 3, 2, 1, 1), record(1, 3, 2, 2, 2)), body=3),))
    # Later stages must reject these roots, symbols, references, and premises;
    # the physically valid sections cannot prevent theory formation.
    yield vector("ground_and_proof_semantics_are_later", functions=(identity,),
                 ground=proposition((record(2, 999, 1, 0),), 999, 0),
                 proofs=certificate((record(1, 0, 1, 99),), (record(2, 0, 99, 999),)))
