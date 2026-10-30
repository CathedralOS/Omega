"""Compare case currentness, observed cleanup, and loan refusal on exact producers."""
import json
from pathlib import Path
import sys

REPOSITORY = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPOSITORY / 'wiki/drafts/measurements/checking_simplification_2026_10_28'))
import validate

CONTROLS = [
    'pass/drops/runtime_cleanup_case_evidence_exit',
    'pass/drops/runtime_selected_case_mutation_complement_exit',
    'pass/drops/runtime_selected_case_mutation_bindings_exit',
    'pass/drops/runtime_nominal_case_observation_exit',
    'pass/drops/runtime_case_selected_cleanup_continuations_exit',
    'fail/cases/nested_payload_read_after_case_replacement',
    'fail/cases/match_guard_invalidates_nested_tag',
    'fail/control_flow/saved_case_predicate_receiver_trailing_write_rejected',
    'fail/control_flow/saved_case_predicate_local_write_retires',
    'fail/control_flow/saved_case_predicate_guard_write_retires',
    'fail/borrows/case_descriptor_scalar_sibling_keeps_live_loan',
    'fail/generics/extraction_inactive_case',
]

if __name__ == '__main__':
    output = Path(sys.argv[3]).resolve()
    validate.CONTROLS = CONTROLS
    validate.FACT = 'saved_case_predicate'
    validate.INVALID_FRAGMENTS = ('cannot prove selected case for payload access',)
    validate.RUNTIME_CONTROLS = {control for control in CONTROLS if control.startswith('pass/')}
    sources = [REPOSITORY / 'tests/omega' / control / 'main.omg' for control in CONTROLS]
    fact = REPOSITORY / 'samples/cli/facts' / validate.FACT
    sources.extend(fact / place / suffix for place in validate.PLACES for suffix in ['main.omg', 'invalid/main.omg'])
    inputs = list(sources)
    for source in sources:
        inputs.extend(source.with_name(name) for name in ['build.omg', 'expected.txt', 'expected_native_exit.txt'] if source.with_name(name).exists())
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
                assert row['authored_oracle_matched'], row
    assert all(validate.benchmark.sha256_file(Path(path)) == digest for path, digest in hashes.items())
    identities = json.loads((output / 'producers.json').read_text())
    for variant, producer in zip('AB', sys.argv[1:3]):
        assert validate.benchmark.sha256_file(Path(producer)) == identities[variant]['sha256']
    validate.save(output / 'inputs.json', hashes)
    validate.save(output / 'results.json', results)
