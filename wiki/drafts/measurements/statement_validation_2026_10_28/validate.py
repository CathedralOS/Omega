"""Reuse the fixed-row validator with four subtraction-specific controls."""
import json
from pathlib import Path
import sys

REPOSITORY = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPOSITORY / 'wiki/drafts/measurements/checking_simplification_2026_10_28'))
import validate

CONTROLS = [
    'pass/arithmetic/runtime_captured_call_result_subtracts_itself_exit',
    'pass/facts/unsigned_exact_subtraction_interval_through_match_join_exit',
    'fail/facts/unsigned_exact_subtraction_interval_through_match_join_rejected',
    'fail/domains/scalar_entry_membership_invalidated_before_subtraction',
]
if __name__ == '__main__':
    output = Path(sys.argv[3]).resolve()
    validate.CONTROLS.extend(CONTROLS)
    validate.main()
    results = json.loads((output / 'results.json').read_text())
    for control in CONTROLS:
        if not control.startswith('fail/'):
            continue
        oracle = REPOSITORY / 'tests/omega' / control / 'expected.txt'
        for row in results:
            if row['case'] == control.replace('/', '-'):
                row['oracle_sha256'] = validate.benchmark.sha256_file(oracle)
                row['oracle'] = oracle.read_text()
                assert all(fragment in row['output'] for fragment in row['oracle'].splitlines() if fragment.strip()), row
    validate.save(output / 'results.json', results)
