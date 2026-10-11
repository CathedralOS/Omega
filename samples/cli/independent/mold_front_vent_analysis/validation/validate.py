"""Run the already-built Omega application sequentially against frozen oracles.
Usage: python3 validation/validate.py /absolute/path/to/native-application
"""
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from reference import ROOT, FIXTURES, INVALID, tiny_inputs, parse, simulate, render, exhaustive_flood

def execute(binary, label, source, expected, code):
    observed = subprocess.run([binary],input=source,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
    if observed.returncode != code or observed.stdout != expected or observed.stderr:
        raise AssertionError((label,observed.returncode,observed.stdout.decode(errors='replace'),observed.stderr))
    return observed.stdout

if __name__=='__main__':
    binary=str(Path(sys.argv[1]).resolve())
    artifact=ROOT/'build'/'validation'
    artifact.mkdir(parents=True,exist_ok=True)
    records=[]
    for name in FIXTURES:
        source=(ROOT/'data'/f'{name}.in').read_bytes()
        expected=(ROOT/'data'/f'{name}.expected').read_bytes()
        # A changed reference must still agree with the independently frozen artifact.
        assert expected==render(simulate(*parse(source.decode()))).encode()
        stdout=execute(binary,name,source,expected,0)
        digest=hashlib.sha256(stdout).hexdigest()
        (artifact/f'{name}.stdout').write_bytes(stdout)
        records.append(dict(case=name,exit=0,stdout_sha256=digest,stderr_bytes=0))
    for name in INVALID:
        source=(ROOT/'data'/f'invalid_{name}.in').read_bytes()
        execute(binary,'invalid_'+name,source,b'INVALID_INPUT\n',64)
    tiny=list(tiny_inputs())
    for number,source in enumerate(tiny):
        expected=(ROOT/'build/tiny-fixtures'/f'tiny_{number:03d}.expected').read_bytes()
        assert expected==render(simulate(*parse(source))).encode()
        execute(binary,f'tiny_{number}',source.encode(),expected,0)
    summary=dict(binary=binary,binary_sha256=hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
                 fixtures=records,invalid_inputs=len(INVALID),tiny_runtime_cases=len(tiny),
                 simple_path_flood_enumerations=exhaustive_flood(),tolerance=0)
    (artifact/'native.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))
