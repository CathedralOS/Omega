#!/usr/bin/env python3
"""Sequential acceptance runner. Does not update oracles or accept reviews."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
from reference import oracle
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[3]
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',required=True,type=Path)
    options=parser.parse_args()
    omega=options.omega.resolve()
    evidence=ROOT/'build'/'validation'; evidence.mkdir(parents=True,exist_ok=True)
    frozen=json.loads((ROOT/'ORACLE_FREEZE.json').read_text())
    assert digest(ROOT/'reference.py')==frozen['reference_sha256'], 'reference changed after freeze'
    for name, expected in frozen['files'].items():
        assert digest(ROOT/'data'/name)==expected, f'frozen oracle changed: {name}'
    fixtures=sorted((ROOT/'data').glob('*.stdin'))
    for fixture in fixtures:
        code, output=oracle(fixture.read_bytes())
        assert code==int(fixture.with_suffix('.exit').read_text()), fixture
        assert output.encode()==fixture.with_suffix('.stdout').read_bytes(), fixture
    environment=os.environ.copy()
    environment['OMEGA_LIBRARY_ROOT']=str(REPOSITORY/'source'/'library')
    results={'base':subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPOSITORY,text=True).strip(),
             'compiler':str(omega),'compiler_sha256':digest(omega),'target':'linux_x86_64',
             'resources':{'sequential':True,'records':256,'line_bytes':64,'timeout_seconds':180},
             'oracle_sha256':digest(ROOT/'ORACLE_FREEZE.json'),'commands':[]}
    def command(name,arguments,payload=b''):
        started=time.monotonic()
        completed=subprocess.run([str(item) for item in arguments],cwd=REPOSITORY,env=environment,input=payload,capture_output=True,timeout=180)
        (evidence/(name+'.stdout')).write_bytes(completed.stdout)
        (evidence/(name+'.stderr')).write_bytes(completed.stderr)
        row={'name':name,'command':[str(item) for item in arguments], 'exit':completed.returncode,'elapsed_seconds':round(time.monotonic()-started,3)}
        results['commands'].append(row)
        (evidence/'results.json').write_text(json.dumps(results,indent=2)+'\n')
        print(f'{name}: exit {completed.returncode}, {row["elapsed_seconds"]}s',flush=True)
        return completed
    check=command('check',[omega,'--check','--target','linux_x86_64',ROOT/'main.omg'])
    assert check.returncode==0,check.stderr.decode(errors='replace')
    native=command('compile',[omega,'--target','linux_x86_64','--build-dir',ROOT/'build'/'native',ROOT/'main.omg'])
    assert native.returncode==0,native.stderr.decode(errors='replace')
    match=re.search(rb'published native output to (.+)',native.stdout)
    assert match, 'compiler did not report a published executable'
    executable=Path(os.fsdecode(match.group(1)))
    results['native_executable']=str(executable)
    results['native_sha256']=digest(executable)
    for fixture in fixtures:
        completed=command('native_'+fixture.stem,[executable],fixture.read_bytes())
        assert completed.returncode==int(fixture.with_suffix('.exit').read_text()),fixture
        assert completed.stdout==fixture.with_suffix('.stdout').read_bytes(), f'output mismatch: {fixture}'
        assert not completed.stderr,fixture
    # The public CLI's comparison proves termination only. Native output is
    # independently compared to the frozen full report on every replay.
    for name in ['loop','fractional_noise','plateau_reversal','reverse_first','ambiguous','bad_token','out_of_range','over_limit']:
        fixture=ROOT/'data'/(name+'.stdin')
        completed=command('both_'+name,[omega,'run','--both',ROOT/'main.omg'],fixture.read_bytes())
        code=int(fixture.with_suffix('.exit').read_text())
        assert completed.returncode==code,completed.stderr.decode(errors='replace')
        assert completed.stdout==fixture.with_suffix('.stdout').read_bytes(),name
        assert f'interp exit: {code}'.encode() in completed.stderr, 'Terminal comparison declined or failed'
        assert b'DIVERGENCE' not in completed.stderr,name
    rejected=command('intended_invalid',[omega,'--check','--target','linux_x86_64',ROOT/'intended_invalid'/'main.omg'])
    assert rejected.returncode==1, 'invalid control must be rejected'
    diagnostic=(rejected.stdout+rejected.stderr).decode(errors='replace')
    expected=(ROOT/'intended_invalid'/'expected.txt').read_text().strip()
    assert expected in diagnostic,diagnostic
    results['accepted']=True
    results['source_hashes']={str(path.relative_to(ROOT)):digest(path) for path in sorted(ROOT.rglob('*.omg')) if 'build' not in path.relative_to(ROOT).parts}
    (evidence/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(f'ACCEPTED: {len(fixtures)} native fixtures, 8 Terminal termination comparisons, intended rejection')
if __name__=='__main__': main()
