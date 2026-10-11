#!/usr/bin/env python3
"""Sequential public-entrypoint validation; never performs package acceptance."""
import argparse, json, os, subprocess, hashlib, sys
from pathlib import Path
from reference import verify, VALID, INVALID
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[3]
parser = argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
parser.add_argument('--both', action='store_true', help='compare native and Terminal termination')
options = parser.parse_args()
output = ROOT/'build'/('comparison' if options.both else 'native')
output.mkdir(parents=True,exist_ok=True)
environment = dict(os.environ,OMEGA_LIBRARY_ROOT=str(REPOSITORY/'source/library'))
records = []
def run(label,command,input_bytes=b''):
    completed = subprocess.run(command,input=input_bytes,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=environment)
    (output/(label+'.stdout')).write_bytes(completed.stdout)
    (output/(label+'.stderr')).write_bytes(completed.stderr)
    records.append({'case':label,'command':command,'exit':completed.returncode})
    (output/'commands.json').write_text(json.dumps(records,indent=2)+'\n')
    return completed
command = [options.omega,'--check',str(ROOT/'main.omg')]
checked = run('check',command)
if checked.returncode:
    sys.exit('check blocked: '+checked.stderr.decode(errors='replace'))
for name in list(VALID)+INVALID:
    command = [options.omega,'run']+(['--both','--keep'] if options.both else [])+[str(ROOT/'main.omg')]
    completed = run(name,command,(ROOT/'data'/(name+'.stdin')).read_bytes())
    expected_exit = 0 if name in VALID else 64
    if completed.returncode != expected_exit:
        sys.exit(f'{name}: exit {completed.returncode}, expected {expected_exit}: '+completed.stderr.decode(errors='replace'))
    verify(name,output/(name+'.stdout'))
    if options.both:
        diagnostic = completed.stderr.decode(errors='replace')
        if 'DECLINED' in diagnostic:
            print(name+': Terminal explicitly declined')
        elif f'interp exit: {expected_exit}' not in diagnostic or 'DIVERGENCE' in diagnostic:
            sys.exit(name+': no explicit Terminal agreement found: '+diagnostic)
print('Validated all runtime cases; logs: '+str(output))
