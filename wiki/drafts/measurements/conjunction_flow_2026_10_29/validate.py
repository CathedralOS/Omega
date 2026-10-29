"""Check the independent-call row, conjunction projection, effects and formation."""
import json
from pathlib import Path
import sys

REPOSITORY = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPOSITORY / 'wiki/drafts/measurements/checking_simplification_2026_10_28'))
import validate

CONTROLS = [
    'pass/core/conjunctive_return_bound_projection_exit',
    'fail/core/conjunctive_return_bound_projection_rejected',
    'pass/core/guaranteed_return_bound_actuals_exit',
    'fail/core/guaranteed_return_bound_actuals_rejected',
    'pass/core/scoped_integer_call_guarantees_composed_exit',
    'pass/core/scoped_integer_call_exact_actual_requires_formation',
    'fail/core/scoped_integer_call_guarantee_requires_formation',
    'fail/core/scoped_integer_call_wrapping_actual_requires_formation',
    'pass/arithmetic/collection_result_bound_exit',
    'fail/arithmetic/collection_result_distinct_windows',
    'fail/arithmetic/collection_result_wrong_argument',
    'fail/arithmetic/collection_result_conditional_bound',
    'fail/arithmetic/collection_result_rebound_descriptor',
    'pass/proofs/piecewise_call_result_arithmetic',
    'pass/proofs/integer_piecewise_result_contract',
    'fail/proofs/piecewise_call_result_not_singleton',
    'pass/arithmetic/runtime_short_circuit_cast_decisions_exit',
    'fail/arithmetic/short_circuit_cast_wrong_path',
    'fail/arithmetic/short_circuit_cast_mutated_subject',
    'pass/proofs/applied_dependent_const_requires',
    'fail/calls/copied_argument_ensures_cannot_restore_callee_mutated_source',
]


if __name__ == '__main__':
    output = Path(sys.argv[3]).resolve()
    validate.CONTROLS = CONTROLS
    validate.FACT = 'independent_call_result_bound'
    validate.INVALID_FRAGMENTS = ('cannot prove ensures contract for exit from', 'forwarded')
    validate.RUNTIME_CONTROLS = {
        'pass/arithmetic/collection_result_bound_exit',
        'pass/core/guaranteed_return_bound_actuals_exit',
        'pass/core/scoped_integer_call_guarantees_composed_exit',
        'pass/core/conjunctive_return_bound_projection_exit',
        'pass/arithmetic/runtime_short_circuit_cast_decisions_exit',
    }
    sources = [REPOSITORY / 'tests/omega' / control / 'main.omg' for control in CONTROLS]
    fact = REPOSITORY / 'samples/cli/facts' / validate.FACT
    sources.extend(fact / place / suffix for place in validate.PLACES for suffix in ['main.omg', 'invalid/main.omg'])
    inputs = sources + [source.with_name('build.omg') for source in sources if source.with_name('build.omg').exists()]
    inputs.extend(source.with_name('expected.txt') for source in sources if source.with_name('expected.txt').exists())
    inputs.extend(source.with_name('expected_native_exit.txt') for source in sources if source.with_name('expected_native_exit.txt').exists())
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
                row['authored_oracle_matched'] = all(fragment in row['output'] for fragment in row['oracle'].splitlines() if fragment.strip())
                if control == 'fail/arithmetic/collection_result_rebound_descriptor':
                    # The fixed baseline already differs from this fixture's old diagnostic.
                    assert row['source_sha256'] == '954b4fae29316b6530db3ca1cc7ef8567cc4407014712eea55821769908418b9', row
                    assert row['oracle_sha256'] == '88cc27c851f706d84569eb06740f89082c82d15c8ce4cdfd47f1a490a03545a1', row
                    assert row['errors'] == ['error: cannot prove index `count` is within length 2 in rejected::entry'], row
                    row['baseline_oracle_mismatch'] = 'Both producers reject the unsafe index; the authored diagnostic is stale.'
                else:
                    assert row['authored_oracle_matched'], row
    assert all(validate.benchmark.sha256_file(Path(path)) == digest for path, digest in hashes.items())
    identities = json.loads((output / 'producers.json').read_text())
    for variant, producer in zip('AB', sys.argv[1:3]):
        assert validate.benchmark.sha256_file(Path(producer)) == identities[variant]['sha256']
    validate.save(output / 'inputs.json', hashes)
    validate.save(output / 'results.json', results)
