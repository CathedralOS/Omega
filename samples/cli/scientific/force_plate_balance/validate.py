#!/usr/bin/env python3
"""Serial public-CLI acceptance checks; run after ordinary package review."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[3]
BINARY = Path(sys.argv[1]).resolve()
BUILD = ROOT / 'build' / 'validation'
BUILD.mkdir(parents=True, exist_ok=True)
ENVIRONMENT = dict(os.environ, OMEGA_LIBRARY_ROOT=str(REPOSITORY / 'source' / 'library'))
records = []

def execute(name, arguments, input_bytes=None, expected_exit=0, expected_stdout=None, fragment=None):
    started = time.monotonic()
    completed = subprocess.run([str(BINARY), *arguments], input=input_bytes, capture_output=True, env=ENVIRONMENT, cwd=REPOSITORY)
    (BUILD / (name+'.stdout')).write_bytes(completed.stdout)
    (BUILD / (name+'.stderr')).write_bytes(completed.stderr)
    record = dict(name=name, command=[str(BINARY),*arguments], exit=completed.returncode, seconds=round(time.monotonic()-started,3))
    records.append(record)
    (BUILD/'commands.json').write_text(json.dumps(records,indent=2)+'\n')
    assert completed.returncode == expected_exit, record
    if expected_stdout is not None:
        assert completed.stdout == expected_stdout, f'{name}: stdout differs from frozen oracle'
    if fragment is not None:
        assert fragment in completed.stderr.decode(errors='replace'), f'{name}: intended rejection missing'
    print(name, 'exit', completed.returncode, flush=True)
    return completed

main = str(ROOT/'main.omg')
execute('check', ['--check','--target','linux_x86_64',main])
for fixture, oracle in [('frames.txt','expected_stdout.txt'),('variants.txt','variants_stdout.txt')]:
    text = (ROOT/'data'/fixture).read_bytes()
    expected = (ROOT/'data'/oracle).read_bytes()
    stem = fixture.removesuffix('.txt')
    execute(stem+'-native',['run','--keep',main],text,expected_stdout=expected)
    both = execute(stem+'-both',['run','--both','--keep',main],text,expected_stdout=expected)
    assert b'interp exit: 0' in both.stderr and b'DECLINED' not in both.stderr, 'Terminal comparison did not agree'
execute('intended-invalid',['--check','--target','linux_x86_64',str(ROOT/'intended_invalid'/'reject.omg')],expected_exit=1,fragment='divisor must be proven nonzero')
subprocess.run([sys.executable,str(ROOT/'data'/'reference.py')],check=True)
print('all serial application checks passed; Terminal status comparison only')
