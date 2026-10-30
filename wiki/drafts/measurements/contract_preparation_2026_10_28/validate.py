"""Check the unchanged stack-call row and focused integer-contract controls."""
import json
from pathlib import Path
import sys

REPOSITORY = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPOSITORY / 'wiki/drafts/measurements/checking_simplification_2026_10_28'))
import validate

CONTROLS = [
    'pass/core/scoped_integer_call_exact_actual_requires_formation',
    'pass/proofs/applied_dependent_const_requires',
    'pass/arithmetic/checked_flag_proof_wrapping_cast_fits',
    'pass/arithmetic/checked_flag_proof_saturating_cast_fits',
    'fail/core/scoped_integer_call_guarantee_requires_formation',
    'fail/core/scoped_integer_call_wrapping_actual_requires_formation',
    'fail/calls/computed_zero_requires_argument_rejected',
    'fail/calls/ordered_bound_requires_argument_rejected',
    'fail/proofs/value_call_refuted_inequality_rejected',
    'fail/calls/closed_constructor_requires_symbolic_unknown',
    'fail/calls/closed_constructor_requires_stale_store_rejected',
    'fail/calls/closed_constructor_requires_restricted_nested_call_rejected',
    'fail/arithmetic/checked_flag_proof_wrapping_cast_overflow',
    'fail/arithmetic/checked_flag_proof_saturating_cast_overflow',
]

if __name__ == '__main__':
    output = Path(sys.argv[3]).resolve()
    validate.CONTROLS = CONTROLS
    sources = [REPOSITORY / 'tests/omega' / control / 'main.omg' for control in CONTROLS]
    fact = REPOSITORY / 'samples/cli/facts/thirtytwo_arg_stack_calling_plan'
    sources.extend(fact / place / suffix for place in validate.PLACES for suffix in ['main.omg', 'invalid/main.omg'])
    inputs = sources + [source.with_name('build.omg') for source in sources if source.with_name('build.omg').exists()]
    inputs.extend(source.with_name('expected.txt') for source in sources if source.with_name('expected.txt').exists())
    hashes = {str(path): validate.benchmark.sha256_file(path) for path in inputs}
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
    assert all(validate.benchmark.sha256_file(Path(path)) == digest for path, digest in hashes.items())
    identities = json.loads((output / 'producers.json').read_text())
    for variant, producer in zip('AB', sys.argv[1:3]):
        assert validate.benchmark.sha256_file(Path(producer)) == identities[variant]['sha256']
    validate.save(output / 'inputs.json', hashes)
    validate.save(output / 'results.json', results)
