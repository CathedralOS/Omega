"""Validate native and captured Terminal numbers against frozen rational oracles."""
import argparse
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parent


def verify(payload, case):
    lines=payload.decode('ascii').splitlines()
    if case['exit']==2:
        assert lines==['invalid input'],lines
        return
    expected=case['expected']
    assert len(lines)==(len(expected['information'])+3 if case['exit']==3 else 2*len(expected['information'])+6),lines
    assert lines[0].split()[0]=='selected'
    assert list(map(int,lines[0].split()[1:]))==expected['selected'],lines[0]
    dimension=len(expected['information'])
    for index,row in enumerate(expected['information']):
        tokens=lines[index+1].split()
        assert tokens[0]=='information' and list(map(int,tokens[1:]))==row,tokens
    tokens=lines[dimension+1].split()
    assert tokens==['determinant',tokens[-1]] and int(tokens[-1])==expected['determinant'],tokens
    assert lines[dimension+2]=='status '+expected['status']
    if case['exit']==3:
        return
    def numeric(actual,target):
        actual=Fraction(actual)
        target=Fraction(target)
        # Stronger than required numerical tolerances: exact rational equality.
        assert actual==target,(actual,target)
        assert math.isclose(float(actual),float(target),rel_tol=1e-8,abs_tol=1e-8)
    for index,row in enumerate(expected['covariance']):
        tokens=lines[dimension+3+index].split()
        assert tokens[0]=='covariance' and len(tokens)==dimension+1,tokens
        for actual,target in zip(tokens[1:],row):
            numeric(actual,target)
    for index,target in enumerate(expected['variances']):
        tokens=lines[2*dimension+3+index].split()
        assert len(tokens)==2 and tokens[0]=='variance',tokens
        numeric(tokens[1],target)


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',required=True)
    parser.add_argument('--native',help='reuse a recorded native executable for fast case runs')
    parser.add_argument('--case',action='append')
    parser.add_argument('--native-only',action='store_true')
    parser.add_argument('--sponsor',help='linked exact-compiler SDK observer; reuses one admitted Terminal artifact')
    options=parser.parse_args()
    if options.sponsor and (not options.native or options.native_only):
        parser.error("--sponsor requires --native and cannot be combined with --native-only")
    if options.native_only and not options.native:
        parser.error("--native-only requires --native; numerical execution cannot be skipped")
    frozen=(ROOT/'data/frozen.json').read_bytes()
    assert hashlib.sha256(frozen).hexdigest()==(ROOT/'data/frozen.sha256').read_text().strip()
    cases=json.loads(frozen)
    if options.case:
        cases=[case for case in cases if case['name'] in options.case]
        assert len(cases)==len(options.case),'unknown or duplicate case'
    output=ROOT/'build'/('sponsor-qualification' if options.sponsor else 'native-qualification' if options.native_only else 'qualification')
    output.mkdir(parents=True,exist_ok=True)
    records=[]
    def execute(name,command,data=b''):
        start=time.monotonic()
        result=subprocess.run(list(map(str,command)),input=data,capture_output=True,timeout=300)
        (output/(name+'.stdout')).write_bytes(result.stdout)
        (output/(name+'.stderr')).write_bytes(result.stderr)
        records.append(dict(name=name,command=list(map(str,command)),exit=result.returncode,
                            seconds=time.monotonic()-start,stdin_sha256=hashlib.sha256(data).hexdigest()))
        (output/'results.json').write_text(json.dumps(records,indent=2)+'\n')
        return result
    if options.sponsor:
        sponsor=Path(options.sponsor).resolve()
        receipt=json.loads(sponsor.with_suffix('.build.json').read_text())
        assert receipt['compiler_binary_sha256']==hashlib.sha256(Path(options.omega).read_bytes()).hexdigest()
        assert receipt['harness_sha256']==hashlib.sha256(sponsor.read_bytes()).hexdigest()
        assert receipt['source_sha256']==hashlib.sha256((ROOT/'validation/terminal_capture.rs').read_bytes()).hexdigest()
        for library in receipt['libraries']:
            assert library['sha256']==hashlib.sha256(Path(library['path']).read_bytes()).hexdigest(),library['name']
        observed=execute('terminal-sponsor',[options.sponsor,ROOT,output/'streams',10000000,30]+[case['name'] for case in cases])
        assert observed.returncode==0,observed.stderr.decode(errors='replace')
    for case in cases:
        payload=(ROOT/'data'/f"{case['name']}.stdin").read_bytes()
        assert payload==case['input'].encode()
        if options.native:
            result=execute('native-'+case['name'],[options.native],payload)
            assert result.returncode==case['exit'],(case['name'],result.stderr)
            verify(result.stdout,case)
        if options.sponsor:
            streams=output/'streams'
            name=case['name']
            assert int((streams/(name+'.terminal.exit')).read_text())==case['exit'],name
            terminal=(streams/(name+'.terminal.stdout')).read_bytes()
            verify(terminal,case)
            assert (streams/(name+'.terminal.stderr')).read_bytes()==b''
            records[-1]['terminal_values_verified']=True
            records[-1]['terminal_sha256']=hashlib.sha256(terminal).hexdigest()
            records[-1]['native_sha256']=hashlib.sha256(result.stdout).hexdigest()
        elif not options.native_only:
            result=execute('both-'+case['name'],[options.omega,'run','--both','--keep',ROOT/'main.omg'],payload)
            assert result.returncode==case['exit'],(case['name'],result.stderr.decode(errors='replace'))
            verify(result.stdout,case)
            diagnostic=result.stderr.decode()
            assert f'interp exit: {case["exit"]}' in diagnostic and 'DECLINED' not in diagnostic and 'DIVERGENCE' not in diagnostic,diagnostic
            kept=re.search(r'build dir kept: (.+)',diagnostic)
            assert kept,'missing retained Terminal streams'
            directory=Path(kept.group(1))
            terminal=(directory/'terminal.stdout').read_bytes()
            verify(terminal,case)
            assert (directory/'terminal.stderr').read_bytes()==b''
            (output/('terminal-'+case['name']+'.stdout')).write_bytes(terminal)
            records[-1]['terminal_values_verified']=True
            records[-1]['terminal_sha256']=hashlib.sha256(terminal).hexdigest()
            records[-1]['native_sha256']=hashlib.sha256(result.stdout).hexdigest()
        print(f"PASS {case['name']}",flush=True)
    control=execute('candidate-bound',[options.omega,'--check',ROOT/'controls/candidate_bound/main.omg'])
    expected=(ROOT/'controls/candidate_bound/expected.txt').read_text().strip()
    diagnostic=('cannot check prepared project: checked compilation failed for package `invalid_candidate_bound` with 1 diagnostic(s)\n  error: '+expected+'\n').encode()
    assert control.returncode==1 and control.stderr==diagnostic and not control.stdout,control.stderr
    (output/'results.json').write_text(json.dumps(records,indent=2)+'\n')
    print(f'PASS {len(cases)} cases; exact selected indices, information, determinant, rational covariance/variance; candidate-bound refusal')

if __name__=='__main__':
    main()
