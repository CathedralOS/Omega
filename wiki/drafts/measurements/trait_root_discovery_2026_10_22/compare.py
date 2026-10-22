"""Compare fresh corpus records with the repository's outcome comparator."""
import collections
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path('tools').resolve()))
import corpus_records

folder = Path('build/root-discovery')
for leg in ('check', 'both', 'policy-check', 'policy-both'):
    baseline_path = (Path('wiki/drafts/measurements/compiler_throughput_2026_10_21') / f'candidate-{leg}.records') if leg in ('check', 'both') else folder / f'baseline-{leg}.records'
    baseline = corpus_records.read(baseline_path)
    candidate = corpus_records.read(folder / f'candidate-{leg}.records')
    assert {row['fixture'] for row in baseline} == {row['fixture'] for row in candidate}, leg
    comparison = corpus_records.compare_records(baseline, candidate)
    previous = {row['fixture']: row for row in baseline}
    comparison['diagnostics_changed'] = [
        row['fixture'] for row in candidate
        if row['fixture'] in previous
        and row['diagnostics'] != previous[row['fixture']]['diagnostics']
    ]
    comparison['facts_changed'] = [row['fixture'] for row in candidate if row['facts'] != previous[row['fixture']]['facts']]
    comparison['candidate_statuses'] = dict(collections.Counter(row['status'] for row in candidate))
    comparison['expected_fragment_failures'] = [row['fixture'] for row in candidate if row['expected_satisfied'] is False]
    comparison['baseline_failures'] = [row['fixture'] for row in baseline
        if row['expected_satisfied'] is False or row['status'] == 'timeout'
        or (row['tier'] != 'fail' and row['status'] not in ('checked', 'built'))
        or (row['tier'] == 'fail' and row['status'] != 'rejected')]
    comparison['agreements'] = sum(row['facts'].get('interpreter') == 'agrees' for row in candidate)
    (folder / f'{leg}-comparison.json').write_text(json.dumps(comparison, indent=2) + '\n')
    print(leg, json.dumps(comparison, indent=2))
    if comparison['losses'] or comparison['gains'] or comparison['diagnostics_changed'] or comparison['facts_changed']:
        raise SystemExit('Observed behavior moved; investigate before retaining this optimization.')
