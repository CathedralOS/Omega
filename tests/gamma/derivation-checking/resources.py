"""Actual full-checker proof tables, never injected comparison state."""

from proof_wire import NAT, ZERO, checked, clause, failure, function, proof_row, record, theory, vector


def cases():
    constant = function((), (clause((ZERO,), body=1),))
    definitions = theory(NAT, (constant,))
    owners = (ZERO, record(2, 1, 0))
    first = record(5, 2, 1, 1)
    reflexivity = record(1, 1, 1)
    count = 163838
    rows = (first,) + (reflexivity,) * (count - 1)
    assert 4 * count + 8 == 655360
    yield vector("exact_complete_checking_work", checked(count, 655360), rows, owners,
                 definitions=definitions, repetitions=1, timeout=600)
    count = 163839
    rows = (first,) + (reflexivity,) * (count - 1)
    # Before final comparisons, P+1 +6 +3(P-1) =4P+4 =655360.
    coordinate = proof_row(rows[:-1], definitions, owners) + 8
    assert coordinate == 2621604
    yield vector("adjacent_final_root_comparison", failure(coordinate, 4, 2, 655360, 655361),
                 rows, owners, definitions=definitions, repetitions=1, timeout=600)
    count = 262143
    # Setup consumes 262144; 131072 Ref rows consume the remaining 393216.
    coordinate = proof_row((reflexivity,) * 131072) + 4
    assert coordinate == 2097268
    yield vector("proof_index_and_rows_share_work",
                 failure(coordinate, 4, 2, 655360, 655361), (reflexivity,) * count,
                 repetitions=1, timeout=600)
    count = 32768
    rows = (reflexivity,) + tuple(record(2, 1, 1, previous) for previous in range(1, count))
    # P+1 setup +3 first row +5(P-1) +4 final =6P+3.
    yield vector("32768_deep_symmetry_proof_dag", checked(count, 196611), rows,
                 repetitions=1, timeout=600)
