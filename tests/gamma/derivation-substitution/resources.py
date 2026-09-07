"""Bulk reservations share the comparison counter and preserve exact requests."""

from substitution_wire import NAT, ZERO, compared, failure, ordinary, record, theory, vector


def cases():
    for selector, amount in ((1, 0), (2, -1), (3, 2147483648), (8, -9223372036854775808)):
        yield vector(f"invalid_bulk_amount_{amount}", failure(907, 11), (ZERO,) * 8,
                     left=selector, right=1, entry="bulk")
    for name, selector, expected in (
        ("small_bulk", 7, compared(1, 1)),
        ("exact_bulk", 5, compared(1, 262144)),
        ("adjacent_bulk", 6, failure(907, 4, 2, 262144, 262145)),
        ("maximum_valid_amount", 4, failure(907, 4, 2, 262144, 2147483647)),
    ):
        yield vector(name, expected, (ZERO,) * 8, left=selector, entry="bulk")
    yield vector("bulk_requested_exceeds_u31", failure(907, 4, 2, 262144, 2147483648),
                 (ZERO,) * 8, left=4, right=3, entry="bulk")
    for selector in (1, 2, 3):
        yield vector(f"invalid_amount_before_exhaustion_{selector}", failure(907, 11),
                     (ZERO,) * 8, left=selector, right=2, entry="bulk")
    yield vector("positive_amount_after_exhaustion", failure(907, 4, 2, 262144, 262145),
                 (ZERO,) * 8, left=7, right=2, entry="bulk")
    owners = (ZERO, record(2, 1, 1, 1), ZERO, ZERO, ZERO, ZERO)
    for selector, name, expected in (
        (1, "exact_unfold_then_adjacent", compared(1, 262144) + failure(907, 4, 2, 262144, 262145)),
        (2, "after_variable_before_terminal_resume", failure(901, 4, 2, 262144, 262145)),
        (3, "template_index_bulk_refuses_exact_sum", failure(901, 4, 2, 262144, 262146)),
        (4, "clause_reservation_owns_coordinate", failure(903, 4, 2, 262144, 262145)),
        (5, "invalid_left_before_exhausted_unfold", failure(901, 9)),
        (6, "invalid_right_before_exhausted_unfold", failure(902, 9)),
    ):
        yield vector(name, expected, owners, right=selector, entry="budget")
    count = 46484
    templates = (record(0, 0),) + tuple(record(1, 2, 1, i) for i in range(1, count))
    owners = (ZERO,) + tuple(record(1, 2, 1, i) for i in range(1, count)) + (record(2, 1, 1, 1),)
    yield vector("46484_deep_template", compared(1, 139456), owners, left=count + 1, right=count,
                 definitions=theory(NAT, (ordinary(templates, (1,), count),)), repetitions=1, timeout=600)
    count = 1024
    constructors = NAT + (record(1, 2, 1, 1),)
    templates = (record(0, 0),) + tuple(record(1, 3, 2, i, i) for i in range(1, count))
    owners = (ZERO,) + tuple(record(1, 3, 2, i, i) for i in range(1, count)) + (record(2, 1, 1, 1),)
    yield vector("shared_template_ground_dags", compared(1, 5122), owners, left=count + 1, right=count,
                 definitions=theory(constructors, (ordinary(templates, (1,), count),)), repetitions=1, timeout=600)
