"""Validate real Omega artifacts without changing or accepting package policy.
Each compiler/executor invocation is sequential. Read all stderr decline notices.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
from reference import reference, render
ROOT = Path(__file__).resolve().parent

parser = argparse.ArgumentParser()
parser.add_argument('--omega', type=Path)
parser.add_argument('--executable', type=Path)
parser.add_argument('--terminal', action='store_true')
options = parser.parse_args()
for line in ((ROOT/'data/oracle.sha256').read_text() + (ROOT/'data/greedy_trap.sha256').read_text()).splitlines():
    expected, name = line.split('  ',1)
    assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest() == expected, name
frozen = json.loads((ROOT/'data/frozen.json').read_text())
frozen['cases']['greedy_trap'] = json.loads((ROOT/'data/greedy_trap_frozen.json').read_text())
evidence = ROOT/'build/evidence'
evidence.mkdir(parents=True,exist_ok=True)
rows = []
def run(label,command,content,stdout,exit_code,terminal=False):
    observed = subprocess.run(command,input=content,capture_output=True,timeout=240)
    (evidence/f'{label}.stdout').write_bytes(observed.stdout)
    (evidence/f'{label}.stderr').write_bytes(observed.stderr)
    row = dict(case=label,command=[str(c) for c in command],exit=observed.returncode,
               stdout_sha256=hashlib.sha256(observed.stdout).hexdigest(),
               stderr_sha256=hashlib.sha256(observed.stderr).hexdigest())
    rows.append(row)
    (evidence/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
    assert observed.returncode==exit_code,(label,'exit',observed.returncode,exit_code,observed.stderr.decode(errors='replace'))
    assert observed.stdout==stdout,(label,'stdout',observed.stdout,stdout)
    if terminal:
        assert b'DECLINED' not in observed.stderr,(label,'Terminal declined',observed.stderr)
        assert f'interp exit: {exit_code}'.encode() in observed.stderr,(label,'no Terminal agreement',observed.stderr)
    else:
        assert not observed.stderr,(label,'stderr',observed.stderr)
    print(f'{label}: exact output and exit {exit_code}',flush=True)

if not options.executable and not options.terminal:
    parser.error('supply --executable and/or --terminal --omega')
for name,case in frozen['cases'].items():
    content=(ROOT/'data'/f'{name}.txt').read_bytes()
    result,matrix,winners=reference(list(map(int,content.split())))
    assert result==case['result'] and matrix==case['matrix']
    assert [list(w) for w in winners]==case['optimal_assignments']
    stdout=(ROOT/'data'/f'{name}.stdout').read_bytes()
    assert render(result).encode()==stdout
    if options.executable:
        run('native_'+name,[options.executable],content,stdout,case['exit'])
    if options.terminal and not name.startswith('synthetic_'):
        assert options.omega
        run('terminal_'+name,[options.omega,'run','--both',ROOT/'main.omg'],content,stdout,case['exit'],True)
for source in sorted((ROOT/'data').glob('invalid_*.txt')):
    if options.executable:
        run('native_'+source.stem,[options.executable],source.read_bytes(),b'INVALID input\n',64)
    if options.terminal:
        run('terminal_'+source.stem,[options.omega,'run','--both',ROOT/'main.omg'],source.read_bytes(),b'INVALID input\n',64,True)
print(f'{len(rows)} real artifact observations verified; oracle unchanged')
