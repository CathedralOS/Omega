"""Run the Omega application against frozen oracles.
Usage: python3 validation/validate.py /absolute/path/to/native-application
       python3 validation/validate.py --omega /absolute/path/to/omega

The second form compiles once with `omega run --both --inputs`, which runs
every fixture, invalid input and tiny case natively and through the Terminal
reference; both engines' stdout, stderr and exit must match.
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

def both(omega):
    """Every case through one compilation and both engines."""
    artifact=ROOT/'build'/'validation'
    inputs=artifact/'both_inputs'; cases=artifact/'both'
    inputs.mkdir(parents=True,exist_ok=True)
    for stale in inputs.glob('*.stdin'): stale.unlink()
    expected={}
    for name in FIXTURES:
        (inputs/f'{name}.stdin').write_bytes((ROOT/'data'/f'{name}.in').read_bytes())
        expected[name]=((ROOT/'data'/f'{name}.expected').read_bytes(),0)
    for name in INVALID:
        (inputs/f'invalid_{name}.stdin').write_bytes((ROOT/'data'/f'invalid_{name}.in').read_bytes())
        expected['invalid_'+name]=(b'INVALID_INPUT\n',64)
    for number,source in enumerate(tiny_inputs()):
        (inputs/f'tiny_{number:03d}.stdin').write_text(source)
        expected[f'tiny_{number:03d}']=(render(simulate(*parse(source))).encode(),0)
    run=subprocess.run([str(Path(omega).resolve()),'run','--both','--inputs',str(inputs),
                        '--outputs',str(cases),str(ROOT/'main.omg')],
                       stdin=subprocess.DEVNULL,capture_output=True,timeout=3600)
    (artifact/'both.cli.stderr').write_bytes(run.stderr)
    assert run.returncode==0,run.stderr.decode(errors='replace')[-2000:]
    rows={row[0]:row for row in (line.split('\t') for line in (cases/'results.tsv').read_text().splitlines()[1:])}
    assert set(rows)==set(expected),(len(rows),len(expected))
    for name,(stdout,code) in expected.items():
        _,native,verdict,interpreter=rows[name]
        assert verdict=='agrees' and int(native)==code==int(interpreter),rows[name]
        for stream in ('','terminal.'):
            assert (cases/f'{name}.{stream}stdout').read_bytes()==stdout,(name,stream)
            assert (cases/f'{name}.{stream}stderr').read_bytes()==b'',(name,stream)
    print(f'{len(expected)} cases verified natively and through the Terminal reference')

if __name__=='__main__' and sys.argv[1:2]==['--omega']:
    both(sys.argv[2])
elif __name__=='__main__':
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
