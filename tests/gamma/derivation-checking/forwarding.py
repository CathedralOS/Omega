"""Earlier owned failures keep their codes and coordinates."""

from proof_wire import ZERO, failure, record, theory, vector


def cases():
    yield "outer_short", b"", failure(0, 1), 2, 60
    yield vector("physical_unknown_rule", failure(116, 6), (record(9, 1, 1),))
    yield vector("formation_before_empty_proof", failure(40, 7), (), definitions=theory((record(0, 0),)))
    yield vector("ground_before_invalid_proof", failure(84, 8), (record(1, 0, 0),),
                 owners=(record(1, 99, 0),))
    yield vector("sort_resource_forwarded", failure(28, 2, 2, 65536, 65537),
                 definitions=theory(sorts=65537))
    constructors = (record(0, 148, *([1] * 148)),) + (record(1, 0),) * 9
    yield vector("formation_work_forwarded", failure(28, 3, 2, 8388608, 8388609),
                 definitions=theory(constructors, sorts=52753))
