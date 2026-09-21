"""Actual full-checker proof tables, never injected comparison state."""

from proof_wire import NAT, ZERO, checked, clause, function, record, theory, vector


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
    # Before final comparisons, P+1 +6 +3(P-1) =4P+4 =655360; the final root
    # comparison adds four, inside the 67,108,864-unit provision.
    yield vector("adjacent_final_root_comparison", checked(count, 655364),
                 rows, owners, definitions=definitions, repetitions=1, timeout=600)
    count = 262143
    # Setup consumes 262144 and each Ref row three more; all rows now complete
    # inside the 67,108,864-unit provision.
    yield vector("proof_index_and_rows_share_work",
                 checked(count, 1048577), (reflexivity,) * count,
                 repetitions=1, timeout=600)
    count = 655360
    # The 130 MiB request extent admits a fresh table whose count+1 index
    # reservation alone requests 655361 units; under the selected bound it
    # completes like the other all-Ref tables at 4P+5 = 2621445 units.
    yield vector("fresh_proof_index_reservation",
                 checked(count, 2621445), (reflexivity,) * count,
                 repetitions=1, timeout=600)
    count = 32768
    rows = (reflexivity,) + tuple(record(2, 1, 1, previous) for previous in range(1, count))
    # P+1 setup +3 first row +5(P-1) +4 final =6P+3.
    yield vector("32768_deep_symmetry_proof_dag", checked(count, 196611), rows,
                 repetitions=1, timeout=600)
