"""Logical depth and sharing are encoded references, not expanded host trees."""

from ground_wire import NAT, ZERO, record, theory, vector


def cases():
    owners = (ZERO,) + tuple(record(1, 2, 1, previous) for previous in range(1, 46484))
    yield vector("46484_owner_row_chain", owners, left=46484, right=46484,
                 repetitions=1, timeout=600)
    # Owner1 remains global1; witness j's predecessor is global j.
    witnesses = tuple(record(1, 2, 1, previous) for previous in range(1, 46485))
    yield vector("46484_witness_row_chain", witnesses=witnesses, repetitions=1, timeout=600)
    branching = theory(NAT + (record(1, 2, 1, 1),))
    shared = (ZERO,) + tuple(record(1, 3, 2, previous, previous) for previous in range(1, 2048))
    yield vector("2048_owner_rows_exponentially_shared_tree", shared, left=2048, right=2048,
                 definitions=branching, repetitions=1, timeout=600)
