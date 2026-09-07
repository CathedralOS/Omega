"""Comparison diagnostics preserve earlier owned failures without comparison."""

from comparison_wire import NAT_THEORY, ZERO, failure, record, theory, vector


def cases():
    yield "short_outer_request", "root", b"", failure(0, 1), 2, 60
    yield vector("formation_failure_forwarded", failure(40, 7), definitions=theory((record(0, 0),)))
    yield vector("sort_provision_forwarded", failure(28, 2, 2, 65536, 65537), definitions=theory(sorts=65537))
    # Theory ends68; owner starts76; symbol84, child92. Root field starts92.
    yield vector("ground_symbol_failure_forwarded", failure(84, 8), (record(1, 99, 0),))
    yield vector("ground_child_failure_forwarded", failure(92, 8), (record(1, 2, 1, 1),))
    yield vector("ground_root_failure_is_not_comparison_reference", failure(92, 8), (ZERO,), left=0)
    yield vector("physical_variable_failure_forwarded", failure(80, 6), (record(0, 0),))
