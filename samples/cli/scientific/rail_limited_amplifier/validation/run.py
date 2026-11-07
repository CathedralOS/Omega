#!/usr/bin/env python3
"""Serial native execution plus independent frozen-oracle stream comparisons."""
import hashlib, json, subprocess, sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT))
sys.dont_write_bytecode=True
import reference
output=Path(sys.argv[1]).resolve()
executable=Path((output/'executable.txt').read_text())
edge=json.loads((ROOT/'data/edge-oracle-freeze.json').read_text())
assert hashlib.sha256((ROOT/'data/oracle-freeze.json').read_bytes()).hexdigest()==edge['original_freeze_sha256']
assert hashlib.sha256((ROOT/'reference.py').read_bytes()).hexdigest()==edge['reference_sha256']
for name,digest in edge['fixtures'].items():
    assert hashlib.sha256((ROOT/'data'/name).read_bytes()).hexdigest()==digest, f'edge oracle modified: {name}'
results=[]
for fixture in sorted((ROOT/'data').glob('*.in')):
    name=fixture.stem
    completed=subprocess.run([str(executable)],input=fixture.read_bytes(),capture_output=True,timeout=30)
    (output/f'{name}.native.stdout').write_bytes(completed.stdout)
    (output/f'{name}.native.stderr').write_bytes(completed.stderr)
    (output/f'{name}.native.exit').write_text(str(completed.returncode)+'\n')
    assert not completed.stderr,f'{name}: unexpected native stderr'
    reference.verify(name,output/f'{name}.native.stdout',str(completed.returncode))
    assert not (output/f'{name}.terminal.stderr').read_bytes(),f'{name}: unexpected Terminal stderr'
    reference.verify(name,output/f'{name}.terminal.stdout',(output/f'{name}.terminal.exit').read_text())
    results.append({'case':name,'native_exit':completed.returncode,'terminal_exit':int((output/f'{name}.terminal.exit').read_text()),'native_rows':len(completed.stdout.splitlines()),'native_stdout_sha256':hashlib.sha256(completed.stdout).hexdigest(),'terminal_stdout_sha256':hashlib.sha256((output/f'{name}.terminal.stdout').read_bytes()).hexdigest()})
(output/'results.json').write_text(json.dumps(results,indent=2)+'\n')
print(f'{len(results)} cases: all native and Terminal outputs satisfy the frozen oracle')
