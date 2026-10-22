"""Capture scoped comparisons with the repository's frozen corpus runners.

Run from the repository root: python3 <this file> baseline|candidate.
Baseline core records are reused from the preceding exact producer, while the
additional calling-policy/projection selection is measured for both producers.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

folder = Path('build/root-discovery').resolve()
previous = Path('wiki/drafts/measurements/compiler_throughput_2026_10_21')
producer = sys.argv[1]
assert producer in ('baseline', 'candidate')
runner = folder / f'corpus-{producer}'
profile = json.loads(subprocess.check_output([str(runner), '--build-profile'], text=True))
assert profile['opt_level'] != '0'
# Reusing the earlier core records requires their exact producer and inputs.
previous_identity = json.loads((previous / 'identities.json').read_text())
if producer == 'baseline':
    assert hashlib.sha256(runner.read_bytes()).hexdigest() == previous_identity['binaries']['corpus-candidate']['sha256']
for tree in ('tests/omega', 'source/library'):
    expected = subprocess.check_output(['git', 'rev-parse', previous_identity['base'] + ':' + tree])
    assert subprocess.check_output(['git', 'rev-parse', 'HEAD:' + tree]) == expected
    assert not subprocess.check_output(['git', 'status', '--porcelain', '--', tree])
selection = {
    'check': (previous / 'filter.txt').read_text().strip(),
    'both': (previous / 'both-filter.txt').read_text().strip(),
    'policy-check': 'facts/generic_calling_policy/,modules/open_const_private_implementation_projection_exit',
    'policy-both': 'facts/generic_calling_policy/,modules/open_const_private_implementation_projection_exit',
}
commands = []
for leg, fixtures in selection.items():
    if producer == 'baseline' and leg in ('check', 'both'):
        continue
    environment = dict(os.environ, OMEGA_CORPUS_ROOT=str(Path.cwd()),
        OMEGA_CORPUS_JOBS='2', OMEGA_CORPUS_FIXTURE_FILTER=fixtures,
        OMEGA_CORPUS_RECORDS=str(folder / f'{producer}-{leg}.records'))
    for name in ('OMEGA_CORPUS_NATIVE', 'OMEGA_CORPUS_INTERPRET',
                 'OMEGA_CORPUS_CHECKED_INTERPRETER', 'OMEGA_CORPUS_SHARD',
                 'OMEGA_CORPUS_MIGRATION_FIXTURES', 'OMEGA_CORPUS_NATIVE_TARGET'):
        environment.pop(name, None)
    if leg.endswith('both'):
        environment.update(OMEGA_CORPUS_NATIVE='1', OMEGA_CORPUS_INTERPRET='1')
    command = [str(runner)]
    with (folder / f'{producer}-{leg}.log').open('w') as output:
        result = subprocess.run(command, env=environment, stdout=output, stderr=subprocess.STDOUT)
    commands.append(dict(producer=producer, leg=leg, command=command,
        selection=fixtures, exit=result.returncode, jobs=2, profile=profile,
        sha256=hashlib.sha256(runner.read_bytes()).hexdigest()))
    (folder / f'{producer}-commands.json').write_text(json.dumps(commands, indent=2)+'\n')
    print(producer, leg, 'runner exit', result.returncode, flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
