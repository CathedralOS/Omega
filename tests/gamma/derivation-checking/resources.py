"""Actual full-checker proof tables, never injected comparison state."""

import os

from proof_wire import (
    NAT, ZERO, checked, clause, failure, function, proof_row, record, theory,
    vector,
)

# No proof-index reservation the 130 MiB extent admits can exhaust the
# 67,108,864-unit work provision (fewer than 8,519,681 minimum-size rows), so
# fresh exhaustion coverage follows the heap/pair veterans' explicit-selection
# pattern: the leg stays pinned with its exact expectation and runs only when
# OMEGA_GAMMA_CHECKING_OPT_IN names it. This gate's runner takes no case
# argument, so selection arrives through the environment; an unknown name
# refuses rather than silently running nothing.
OPT_IN = "OMEGA_GAMMA_CHECKING_OPT_IN"


def opt_in(name):
    if name != "checking_work_provision_exhaustion":
        raise SystemExit(f"Derivation checking: unknown {OPT_IN} leg {name!r}")
    # One mode-0 clause of 524,282 single-leaf template rows charges
    # T+5 = 524,287 units per Unfolding row: the rule-field and clause-scan
    # reservations, the T+1 = 524,283 template-index provision, and the two
    # visit/resume transitions over the leaf body. Setup and 127 identical
    # rows hold 66,584,578; row 128's index provision lands the counter on
    # 67,108,863, the body's visit consumes unit 67,108,864, and the resume
    # requests 67,108,865. Substitution work refusals report the row's left
    # field; the clause-scan reservation reports its clause field.
    depth = 524282
    leaves = (record(1, 1, 0),) * depth
    giant = clause(leaves, constructor=0, body=1)
    definitions = theory(NAT, (function((), (giant,), mode=0),))
    owners = (ZERO, record(2, 1, 0))
    rows = (record(5, 2, 1, 1),) * 128
    coordinate = proof_row(rows[:-1], definitions, owners) + 8
    assert coordinate == 8391228
    yield vector("checking_work_provision_exhaustion",
                 failure(coordinate, 4, 2, 67108864, 67108865),
                 rows, owners, left=2, right=1,
                 definitions=definitions, repetitions=1, timeout=43200)


def cases():
    selected = os.environ.get(OPT_IN)
    if selected is not None:
        yield from opt_in(selected)
        return
    print("Derivation checking: skipping the work-provision exhaustion leg "
          "(unreachable in gate time; OMEGA_GAMMA_CHECKING_OPT_IN="
          "checking_work_provision_exhaustion to run)", flush=True)
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
