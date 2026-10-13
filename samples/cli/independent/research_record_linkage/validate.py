"""Validate real Omega artifacts without changing or accepting package policy.

`--terminal --omega PATH` compiles once with `omega run --both --input ...`
over every data file, which runs each case natively and through the Terminal
reference; both engines' stdout, stderr and exit must match the oracle.
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
def run(label,command,content,stdout,exit_code):
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
    assert not observed.stderr,(label,'stderr',observed.stderr)
    print(f'{label}: exact output and exit {exit_code}',flush=True)

if not options.executable and not options.terminal:
    parser.error('supply --executable and/or --terminal --omega')
both = {}
if options.terminal:
    assert options.omega
    cases_dir = evidence/'both'
    command = [str(options.omega), 'run', '--both']
    for source in sorted((ROOT/'data').glob('*.txt')):
        command += ['--input', str(source)]
    command += ['--outputs', str(cases_dir), str(ROOT/'main.omg')]
    observed = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True, timeout=1800)
    (evidence/'both.cli.stderr').write_bytes(observed.stderr)
    assert observed.returncode == 0, observed.stderr.decode(errors='replace')
    both = {row[0]: row for row in (line.split('\t') for line in (cases_dir/'results.tsv').read_text().splitlines()[1:])}

def check_both(name, stdout, exit_code):
    _, native, verdict, interpreter = both[name]
    assert verdict == 'agrees' and int(native) == exit_code == int(interpreter), both[name]
    for stream in ('', 'terminal.'):
        assert (evidence/'both'/f'{name}.{stream}stdout').read_bytes() == stdout, (name, stream)
        assert not (evidence/'both'/f'{name}.{stream}stderr').read_bytes(), (name, stream)
    rows.append(dict(case='both_'+name, exit=exit_code))
    print(f'{name}: native and Terminal exact output and exit {exit_code}', flush=True)
for name,case in frozen['cases'].items():
    content=(ROOT/'data'/f'{name}.txt').read_bytes()
    result,matrix,winners=reference(list(map(int,content.split())))
    assert result==case['result'] and matrix==case['matrix']
    assert [list(w) for w in winners]==case['optimal_assignments']
    stdout=(ROOT/'data'/f'{name}.stdout').read_bytes()
    assert render(result).encode()==stdout
    if options.executable:
        run('native_'+name,[options.executable],content,stdout,case['exit'])
    if options.terminal:
        check_both(name, stdout, case['exit'])
for source in sorted((ROOT/'data').glob('invalid_*.txt')):
    if options.executable:
        run('native_'+source.stem,[options.executable],source.read_bytes(),b'INVALID input\n',64)
    if options.terminal:
        check_both(source.stem, b'INVALID input\n', 64)
print(f'{len(rows)} real artifact observations verified; oracle unchanged')
