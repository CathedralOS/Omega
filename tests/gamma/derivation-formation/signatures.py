"""Signature-first ordering and finite inhabitation admission."""

from theory_wire import NAT, ZERO, failure, function_start, ordinary, record, vector


def cases():
    yield vector("zero_sorts", constructors=(), sorts=0, expected=failure(28))
    for value in (0, 2):
        yield vector(f"constructor_result_{value}", (record(value, 0),), expected=failure(40))
        yield vector(f"constructor_argument_{value}", (record(1, 1, value),), expected=failure(48))
        offset = function_start(NAT)
        yield vector(f"function_result_{value}", functions=(ordinary((ZERO,), result=value),),
                     expected=failure(offset + 4))
        yield vector(f"function_argument_{value}", functions=(ordinary((ZERO,), (value,)),),
                     expected=failure(offset + 12))
    yield vector("signature_result_before_argument", (record(0, 1, 0),), expected=failure(40))
    yield vector("signature_argument_order", (record(1, 3, 1, 0, 2),), expected=failure(52))
    for index in (1, 2):
        arguments = [1, 1, 1]
        arguments[index] = 0
        yield vector(f"function_signature_argument_position_{index}",
                     functions=(ordinary((ZERO,), tuple(arguments)),),
                     expected=failure(function_start(NAT) + 12 + 4 * index))
    yield vector("function_result_before_its_arguments", functions=(ordinary((ZERO,), (0,), result=0),),
                 expected=failure(function_start(NAT) + 4))
    constructors = (record(1, 0), record(0, 0))
    yield vector("constructor_order_before_function_signatures", constructors,
                 (ordinary((ZERO,), result=0),), expected=failure(52))
    first = ordinary((record(0, 99),))
    second = ordinary((ZERO,), result=0)
    yield vector("all_signatures_before_earlier_body", functions=(first, second),
                 expected=failure(function_start(NAT, (first,)) + 4))
    cycle = (record(1, 1, 1),)
    yield vector("function_signature_before_inhabitation", cycle,
                 (ordinary((ZERO,), result=0),), expected=failure(function_start(cycle) + 4))
    yield vector("empty_constructor_catalog", (), expected=failure(28))
    yield vector("uninhabited_self_cycle", cycle, expected=failure(28))
    yield vector("uninhabited_mutual_cycle", (record(1, 1, 2), record(2, 1, 1)),
                 sorts=2, expected=failure(28))
    yield vector("one_sort_without_inhabitant", (record(1, 0),), sorts=2, expected=failure(28))
    yield vector("inhabitation_before_body", cycle, (first,), expected=failure(28))
