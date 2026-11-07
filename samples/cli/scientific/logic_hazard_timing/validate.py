"""Run sequential product checks; exact native stdout and authored Terminal checks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
options = parser.parse_args()
binary = Path(options.omega).resolve()
out = ROOT/'build'/'validation'
out.mkdir(parents=True, exist_ok=True)
records = []
def invoke(name, arguments, stdin=b'', expected_exit=0, expected_stdout=None):
    started = time.monotonic()
    process = subprocess.run([str(binary), *arguments], input=stdin,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             cwd=ROOT, timeout=300)
    (out/f'{name}.stdout').write_bytes(process.stdout)
    (out/f'{name}.stderr').write_bytes(process.stderr)
    okay = process.returncode == expected_exit
    if expected_stdout is not None: okay = okay and process.stdout == expected_stdout
    if '--both' in arguments:
        okay = okay and f'interp exit: {expected_exit}\n'.encode() in process.stderr and b'DECLINED' not in process.stderr and b'DIVERGENCE' not in process.stderr
    records.append(dict(name=name, command=[str(binary), *arguments],
                        input_sha256=hashlib.sha256(stdin).hexdigest(),
                        exit=process.returncode, expected_exit=expected_exit,
                        okay=okay, seconds=round(time.monotonic()-started,3)))
    print(name, 'PASS' if okay else 'FAIL', process.returncode, flush=True)
    if not okay: print(process.stderr.decode(errors='replace'), flush=True)
    return process

invoke('check', ['--check', str(ROOT/'main.omg')])
for mode in range(3):
    expected = (ROOT/'data'/f'mode{mode}.expected').read_bytes()
    input_bytes = (ROOT/'data'/f'mode{mode}.input').read_bytes()
    invoke(f'native{mode}', ['run', str(ROOT/'main.omg')], input_bytes, 0, expected)
    invoke(f'both{mode}', ['run', '--both', str(ROOT/'main.omg')], input_bytes, 0, expected)
header = (ROOT/'data'/'mode0.expected').read_bytes().split(b'\n')[0]+b'\n'
for name,stdin in [('unknown',b'x'),('empty',b'')]:
    expected = header+b'REJECT input: expected one byte 0,1,2,d\n'
    invoke(name, ['run', '--both', str(ROOT/'main.omg')], stdin, 2, expected)
invoke('invalid_delay', ['run','--both',str(ROOT/'main.omg')], b'd', 3,
       header+b'REJECT netlist: positive delay <=8 and acyclic valid gates required\n')
rejection = invoke('invalid_source', ['--check',str(ROOT/'invalid'/'main.omg')], expected_exit=1)
# Require the declared obligation's diagnostic, not an unrelated package failure.
records[-1]['okay'] = records[-1]['okay'] and b'delay' in rejection.stderr and any(
    phrase in rejection.stderr.lower() for phrase in [b'cannot prove',b'disproved',b'precondition'])
summary = dict(binary=str(binary), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
               base=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
               uid=os.getuid() if hasattr(os,'getuid') else None,
               host=dict(zip(('system','node','release','version','machine'),os.uname())) if hasattr(os,'uname') else {},
               resources={'rust_build_jobs':4,'omega_execution_concurrency':1,'cgroup_memory_bytes':17179869184,'cpu_quota_cores':4},
               sources={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest()
                        for p in sorted(ROOT.rglob('*')) if p.is_file()
                        and not {'build','__pycache__'}.intersection(p.relative_to(ROOT).parts)
                        and p.name != 'omega.lock'},
               records=records)
(out/'results.json').write_text(json.dumps(summary,indent=2)+'\n')
raise SystemExit(0 if all(r['okay'] for r in records) else 1)
