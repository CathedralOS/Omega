"""Verify the complete archived observation against source pins and frozen values."""
import hashlib
import json
from check_output import ROOT, check

receipt = json.loads((ROOT / 'validation/observed.json').read_text())
oracle = json.loads((ROOT / 'validation/oracle.json').read_text())
assert receipt['schema'] == 'bio02-observed-v1'
assert hashlib.sha256((ROOT / 'validation/oracle.json').read_bytes()).hexdigest() == receipt['oracle_sha256']
for name, expected in receipt['inputs'].items():
    assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected, name
repair = receipt['compiler_repair']
assert hashlib.sha256((ROOT.parents[3] / repair['path']).read_bytes()).hexdigest() == repair['sha256']
for mode in ('native', 'terminal'):
    assert set(receipt[mode]) == set(oracle), 'incomplete archived fixture set'
    for name, row in receipt[mode].items():
        assert hashlib.sha256(row['stdout'].encode()).hexdigest() == row['stdout_sha256']
        check(name, row['stdout'], row['exit'])
    print(mode, 'verified all', len(oracle), 'archived numerical observations')
