"""Whole-session exact work boundaries and deep/shared structural comparisons."""

from comparison_wire import NAT, ZERO, compared, failure, record, theory, vector


def cases():
    different = theory((record(1, 0), record(1, 0)))
    owners = (ZERO, record(1, 2, 0))
    yield vector("exact_then_adjacent_transition",
                 compared(1, 262144) + failure(4097, 4, 2, 262144, 262145), owners,
                 right=2, definitions=different, entry="budget", repetitions=1, timeout=600)
    yield vector("invalid_ids_precede_exhausted_counter",
                 compared(1, 262144) + failure(4097), owners,
                 right=1, definitions=different, entry="budget", repetitions=1, timeout=600)
    yield vector("terminal_resume_cannot_return_boolean_at_exhaustion",
                 compared(0, 262143) + failure(12289, 4, 2, 262144, 262145), owners,
                 right=2, definitions=different, entry="resume", repetitions=1, timeout=600)
    yield vector("pending_parent_cannot_complete_at_exhaustion",
                 failure(20481, 4, 2, 262144, 262145),
                 (ZERO, record(1, 2, 1, 1), record(1, 2, 1, 1)), left=2, right=3,
                 entry="pending", repetitions=1, timeout=600)
    count = 46484
    first = (ZERO,) + tuple(record(1, 2, 1, index) for index in range(1, count))
    second = (ZERO,) + tuple(record(1, 2, 1, count + index) for index in range(1, count))
    # Each distinct unary node has one visit and one resume: 2*46484 transitions.
    yield vector("two_46484_deep_chains", compared(1, 92968), first + second,
                 left=count, right=2 * count, repetitions=1, timeout=600)
    count = 1024
    branching = theory(NAT + (record(1, 2, 1, 1),))
    first = (ZERO,) + tuple(record(1, 3, 2, index, index) for index in range(1, count))
    second = (ZERO,) + tuple(record(1, 3, 2, count + index, count + index)
                              for index in range(1, count))
    # Nullary base2; each level adds parent visit/resume and a memo child visit/resume.
    yield vector("separately_encoded_shared_dags", compared(1, 4094), first + second,
                 left=count, right=2 * count, definitions=branching, repetitions=1, timeout=600)
