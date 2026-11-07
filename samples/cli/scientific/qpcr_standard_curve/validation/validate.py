"""Validate actual native and retained Terminal numeric streams, never exits alone."""
import argparse
from decimal import Decimal, localcontext
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
sys.dont_write_bytecode=True
from reference import oracle
ROOT=Path(__file__).resolve().parents[1]

def execute(command, text, directory):
    result=subprocess.run(command,input=text.encode(),capture_output=True,timeout=90)
    directory.mkdir(parents=True,exist_ok=True)
    (directory/'native.stdout').write_bytes(result.stdout)
    (directory/'cli.stderr').write_bytes(result.stderr)
    (directory/'native.status').write_text(str(result.returncode))
    matches=re.findall(rb'build dir kept: (.+)',result.stderr)
    assert len(matches)==1,(command,result.returncode,result.stderr.decode())
    retained=Path(matches[0].decode())
    for name in ['terminal.stdout','terminal.stderr']:
        data=(retained/name).read_bytes()
        (directory/name).write_bytes(data)
    (directory/'retained-path.txt').write_text(str(retained))
    assert not (directory/'terminal.stderr').read_bytes()
    return result

def observe(data):
    rows={}
    for line in data.decode().splitlines():
        match=re.fullmatch(r'([a-z0-9_]+)=(-?[0-9]+)@(-?[0-9]+)',line)
        assert match,line
        key,significand,exponent=match.groups()
        rows.setdefault(key,[]).append(Decimal(significand)*Decimal(10)**int(exponent))
    return rows

def invalids():
    good='3 1\n1 40 100 33 10000 26\n30 0.5\n'
    cases=[('',10),('2 0\n1 40 10 35\n',10),('9 0\n',10),('3 9\n',10),('3.5 0\n',10),('3 -1\n',10),('3 0.5\n',10)]
    for old,new in [('1 40','0 40'),('1 40','-1 40'),('1 40','1000001 40'),('1 40','1 51'),('1 40','1 -1'),('30 0.5','51 0.5'),('30 0.5','-1 0.5'),('30 0.5','30 -0.1')]:
        cases.append((good.replace(old,new),10))
    for bad in ['nan','inf','-inf','1e3','+1','1.','.5','1..5','-']:
        cases.append((good.replace('100 33',bad+' 33'),10))
    cases += [('3 0\n148631 0.2 522220 0.2 845253 0.2\n',11),('6 0\n10 27.663 10 47.569 10 1.903 10 34.578 10 14.528 10 28.697\n',11),(good+'extra',10),(good[:-7],10),(good+' '*4096,10),(' '*4096+good,10),('3 0\n10 30 10 29 10 28\n',11),('3 0\n1 30 10 31 100 32\n',11),('3 0\n1 30 10 30 100 30\n',11),('3 0\n1 30 10 29.99999 100 29.99998\n',12),(good.replace('30 0.5','30 1000000'),12)]
    cases += [(case['input'], case['exit']) for case in json.loads((ROOT/'validation/supplemental-oracles.json').read_text()) if case['exit'] != 0]
    return cases

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',required=True)
    parser.add_argument('--route',choices=['all'],required=True)
    parser.add_argument('--evidence',type=Path,default=ROOT/'build/validation')
    args=parser.parse_args()
    for line in (ROOT/'validation/FROZEN.sha256').read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        path = ROOT/'validation'/Path(name).name
        assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, ('frozen manifest mismatch',path)
    cases=json.loads((ROOT/'validation/oracles.json').read_text())
    cases += [case for case in json.loads((ROOT/'validation/supplemental-oracles.json').read_text()) if case['exit'] == 0]
    for case in cases:
        assert oracle(case['input'])==case['expected'],'frozen oracle drift before execution'
    files=sorted(ROOT.rglob('*.omg'))+[ROOT/'validation/oracles.json',ROOT/'validation/reference.py',ROOT/'validation/supplemental-oracles.json',ROOT/'validation/FROZEN.sha256',Path(__file__).resolve(),Path(args.omega).resolve()]
    identities={str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
    args.evidence.mkdir(parents=True,exist_ok=True)
    (args.evidence/'preexecution-identities.json').write_text(json.dumps(identities,indent=2)+'\n')
    command=[str(Path(args.omega).resolve()),'run','--both','--keep',str(ROOT/'main.omg')]
    with localcontext() as context:
        context.prec=90
        for case in cases:
            directory=args.evidence/case['name']
            result=execute(command,case['input'],directory)
            assert result.returncode==0 and b'interp exit: 0' in result.stderr,(case['name'],result.stderr)
            for engine in ['native','terminal']:
                actual=observe((directory/f'{engine}.stdout').read_bytes())
                assert set(actual)=={key for key, values in case['expected'].items() if values},(case['name'],engine,actual)
                for key,expected in case['expected'].items():
                    assert len(actual.get(key,[]))==len(expected),(case['name'],key)
                    for measured,frozen in zip(actual.get(key,[]),expected):
                        wanted=Decimal(frozen)
                        tolerance=abs(wanted)*Decimal('1e-6') if key in ['copies','lower','upper'] else Decimal('1e-8') * max(abs(wanted), Decimal(1)) if key == 'efficiency' else Decimal('1e-8')
                        if 'extrapolated' in key: tolerance=Decimal(0)
                        assert abs(measured-wanted)<=tolerance,(case['name'],engine,key,measured,wanted)
                print(case['name'],engine,'numerical values qualified')
        for index,(text,exit_code) in enumerate(invalids()):
            directory=args.evidence/f'invalid-{index}'
            result=execute(command,text,directory)
            assert result.returncode==exit_code,(index,result.returncode,result.stderr)
            assert f'interp exit: {exit_code}'.encode() in result.stderr,(index,result.stderr)
            native=(directory/'native.stdout').read_bytes()
            assert native.startswith(b'ERROR ') or b'\nERROR ' in native,(index,native)
            assert native==(directory/'terminal.stdout').read_bytes(),index
        control=ROOT/'controls/standard_bound/main.omg'
        result=subprocess.run([args.omega,'--check',str(control)],input=b'',capture_output=True,timeout=90)
        (args.evidence/'source-invalid.stderr').write_bytes(result.stderr)
        assert result.returncode==1 and b'cannot prove index `8` is within length 8' in result.stderr,result.stderr
    assert identities=={str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in files},'inputs changed'
    summary=dict(numerical=len(cases),runtime_invalid=len(invalids()),source_invalid=1,engines=['native','terminal'],producer=str(Path(args.omega).resolve()))
    (args.evidence/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(summary)
if __name__=='__main__': main()
