"""Check every captured Terminal stream with the original Decimal oracle."""
import csv
import hashlib
import json
from decimal import localcontext
from pathlib import Path

from check_output import ROOT, check

destination = ROOT / 'build/validation/terminal-api'
records = list(csv.DictReader((destination / 'records.tsv').open(), delimiter='\t'))
oracle = json.loads((ROOT / 'validation/oracle.json').read_text())
assert len(records) == len(oracle['cases'])
assert {row['case'] for row in records} == set(oracle['cases'])
for row in records:
    name = row['case']
    output = destination / (name + '.stdout')
    with localcontext() as context:
        context.prec = 70
        check(name, output, int(row['exit']))
    assert (destination / (name + '.stderr')).read_bytes() == b''
    row['stdout_sha256'] = hashlib.sha256(output.read_bytes()).hexdigest()
    row['byte_equal_to_native'] = output.read_bytes() == (ROOT / 'build/validation/native' / (name + '.stdout')).read_bytes()
    print('verified Terminal CSV ' + name)
artifact = destination / 'application.terminal'
assert artifact.stat().st_size > 100
(destination / 'verification.json').write_text(json.dumps({
    'oracle_sha256': hashlib.sha256((ROOT / 'validation/oracle.json').read_bytes()).hexdigest(),
    'terminal_artifact_sha256': hashlib.sha256(artifact.read_bytes()).hexdigest(),
    'cases': records,
}, indent=2) + '\n')
