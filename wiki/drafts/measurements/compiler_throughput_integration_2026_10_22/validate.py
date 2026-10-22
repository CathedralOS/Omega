"""Compare the integrated compiler against its exact current-main producer."""
from pathlib import Path
import collections
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, str(Path('tools').resolve()))
import corpus_records
folder = Path('build/throughput-final').resolve()
producer, leg = sys.argv[1:]
assert producer in ('baseline', 'candidate') and leg in ('check', 'both')
runner = folder / f'corpus-{producer}'
profile = json.loads(subprocess.check_output([str(runner), '--build-profile'], text=True))
assert profile == {'schema': 'omega-compiler-build-profile-v1', 'opt_level': '1'}
common = 'generics/owned_member_,providers/resource_profile_roles'
selection = (folder / ('filter.txt' if leg == 'check' else 'both-filter.txt')).read_text().strip() + ',' + common
if leg == 'both':
    selection += ',generics/owned_member_,providers/resource_profile_roles'
environment = dict(os.environ)
for name in list(environment):
    if name.startswith('OMEGA_CORPUS_'):
        environment.pop(name)
environment.update(OMEGA_CORPUS_ROOT=str(Path.cwd()), OMEGA_CORPUS_JOBS='2',
    OMEGA_CORPUS_FIXTURE_FILTER=selection,
    OMEGA_CORPUS_RECORDS=str(folder / f'{producer}-{leg}.records'))
if leg == 'both':
    environment.update(OMEGA_CORPUS_NATIVE='1', OMEGA_CORPUS_INTERPRET='1')
command = [str(runner)]
with (folder / f'{producer}-{leg}.log').open('w') as output:
    result = subprocess.run(command, env=environment, stdout=output, stderr=subprocess.STDOUT)
metadata = dict(command=command, selection=selection, jobs=2, exit=result.returncode,
    runner_sha256=hashlib.sha256(runner.read_bytes()).hexdigest(), profile=profile,
    observed_head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip())
(folder / f'{producer}-{leg}-command.json').write_text(json.dumps(metadata,indent=2)+'\n')
assert result.returncode == 0
rows = corpus_records.read(folder / f'{producer}-{leg}.records')
print(producer, leg, len(rows), dict(collections.Counter(row['status'] for row in rows)), flush=True)
if producer == 'candidate':
    baseline = corpus_records.read(folder / f'baseline-{leg}.records')
    previous = {row['fixture']: row for row in baseline}
    assert previous.keys() == {row['fixture'] for row in rows}
    comparison = corpus_records.compare_records(baseline, rows)
    comparison['diagnostics_changed'] = [row['fixture'] for row in rows if row['diagnostics'] != previous[row['fixture']]['diagnostics']]
    comparison['facts_changed'] = [row['fixture'] for row in rows if row['facts'] != previous[row['fixture']]['facts']]
    comparison['statuses'] = dict(collections.Counter(row['status'] for row in rows))
    comparison['agreements'] = sum(row['facts'].get('interpreter') == 'agrees' for row in rows)
    comparison['baseline_issues'] = [row['fixture'] for row in baseline
        if row['expected_satisfied'] is False or row['status'] == 'timeout'
        or (row['tier'] != 'fail' and row['status'] not in ('checked', 'built'))
        or (row['tier'] == 'fail' and row['status'] != 'rejected')]
    (folder / f'{leg}-comparison.json').write_text(json.dumps(comparison,indent=2)+'\n')
    print(json.dumps(comparison,indent=2))
    assert not any(comparison[name] for name in ('losses','gains','diagnostics_changed','facts_changed'))
