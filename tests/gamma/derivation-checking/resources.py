"""Actual full-checker proof tables, never injected comparison state."""

from proof_wire import NAT, ZERO, checked, clause, failure, function, proof_count, proof_row, record, theory, vector


def cases():
    constant = function((), (clause((ZERO,), body=1),))
    definitions = theory(NAT, (constant,))
    owners = (ZERO, record(2, 1, 0))
    first = record(5, 2, 1, 1)
    reflexivity = record(1, 1, 1)
    count = 65534
    rows = (first,) + (reflexivity,) * (count - 1)
    assert 4 * count + 8 == 262144
    yield vector("exact_complete_checking_work", checked(count, 262144), rows, owners,
                 definitions=definitions, repetitions=1, timeout=600)
    count = 65535
    rows = (first,) + (reflexivity,) * (count - 1)
    # Before final comparisons, P+1 +6 +3(P-1) =4P+4 =262144.
    coordinate = proof_row(rows[:-1], definitions, owners) + 8
    assert coordinate == 1048740
    yield vector("adjacent_final_root_comparison", failure(coordinate, 4, 2, 262144, 262145),
                 rows, owners, definitions=definitions, repetitions=1, timeout=600)
    count = 262143
    yield vector("proof_index_exact_then_first_row_refusal",
                 failure(proof_row() + 4, 4, 2, 262144, 262145), (reflexivity,) * count,
                 repetitions=1, timeout=600)
    count = 262144
    yield vector("adjacent_proof_index_reservation",
                 failure(proof_count(), 4, 2, 262144, 262145), (reflexivity,) * count,
                 repetitions=1, timeout=600)
    count = 32768
    rows = (reflexivity,) + tuple(record(2, 1, 1, previous) for previous in range(1, count))
    # P+1 setup +3 first row +5(P-1) +4 final =6P+3.
    yield vector("32768_deep_symmetry_proof_dag", checked(count, 196611), rows,
                 repetitions=1, timeout=600)
