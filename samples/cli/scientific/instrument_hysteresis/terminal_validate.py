#!/usr/bin/env python3
"""Use the stable corpus runner to assert native AND Terminal report bytes.
Stages byte-identical application modules in temporary corpus fixtures and
removes them on completion; it does not change or pin repository goldens.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[3]
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--runner',required=True,type=Path)
    options=parser.parse_args()
    runner=options.runner.resolve()
    sys.path.insert(0,str(REPOSITORY/'tools'))
    import corpus_records
    evidence=ROOT/'build'/'terminal_validation'; evidence.mkdir(parents=True,exist_ok=True)
    freeze=json.loads((ROOT/'ORACLE_FREEZE.json').read_text())
    for name, expected in freeze['files'].items():
        assert digest(ROOT/'data'/name)==expected, name
    names=['loop','fractional_noise','plateau_reversal','reverse_first','ambiguous','bad_token','out_of_range','over_limit']
    sources=[ROOT/'main.omg',*sorted((ROOT/'domain').glob('*.omg')),*sorted((ROOT/'io').glob('*.omg'))]
    source_hashes={str(path.relative_to(ROOT)):digest(path) for path in sources}
    with tempfile.TemporaryDirectory(prefix='ind15_terminal_',dir=REPOSITORY/'tests'/'omega'/'pass') as temporary:
        staging=Path(temporary)
        for name in names:
            fixture=staging/name;fixture.mkdir()
            for source in sources:
                destination=fixture/source.relative_to(ROOT);destination.parent.mkdir(parents=True,exist_ok=True)
                shutil.copyfile(source,destination)
                assert digest(destination)==digest(source)
            build=(ROOT/'build.omg').read_text()
            build=build.replace('../../../../source/library/std',os.path.relpath(REPOSITORY/'source'/'library'/'std',fixture))
            (fixture/'build.omg').write_text(build)
            shutil.copyfile(ROOT/'data'/(name+'.stdin'),fixture/'input.txt')
            shutil.copyfile(ROOT/'data'/(name+'.stdout'),fixture/'expected_stdout.txt')
            shutil.copyfile(ROOT/'data'/(name+'.exit'),fixture/'expected_native_exit.txt')
            (fixture/'expected_stderr.txt').write_bytes(b'')
        env=os.environ.copy()
        env.update(OMEGA_LIBRARY_ROOT=str(REPOSITORY/'source'/'library'),OMEGA_CORPUS_JOBS='1',OMEGA_CORPUS_FIXTURE_SECS='180',
                   OMEGA_CORPUS_NATIVE='1',OMEGA_CORPUS_NATIVE_TARGET='linux_x86_64',OMEGA_CORPUS_INTERPRET='1',
                   OMEGA_CORPUS_FIXTURE_FILTER='pass/'+staging.name,OMEGA_CORPUS_RECORDS=str(evidence/'records.txt'))
        # This is Terminal, never the optional checked-tree interpreter.
        env.pop('OMEGA_CORPUS_CHECKED_INTERPRETER',None)
        completed=subprocess.run([str(runner)],cwd=REPOSITORY,env=env,capture_output=True,timeout=180*len(names)+60)
        (evidence/'runner.stdout').write_bytes(completed.stdout)
        (evidence/'runner.stderr').write_bytes(completed.stderr)
        assert completed.returncode==0,completed.stderr.decode(errors='replace')
        records=corpus_records.read(evidence/'records.txt')
        assert len(records)==len(names),records
        for row in records:
            name=row['fixture'].rsplit('/',1)[1]
            expected=int((ROOT/'data'/(name+'.exit')).read_text())
            assert row['status']=='built',row
            for key,value in {'exit':str(expected),'stdout':'match','stderr':'match','interpreter-exit':str(expected),
                              'interpreter':'agrees','interpreter-stdout':'match','interpreter-stderr':'match'}.items():
                assert row['facts'].get(key)==value,(row,key,value)
        (evidence/'results.json').write_text(json.dumps({'accepted':True,'runner':str(runner),'runner_sha256':digest(runner),
             'base':subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPOSITORY,text=True).strip(),
             'target':'linux_x86_64','jobs':1,'engine':'Terminal','source_hashes':source_hashes,
             'oracle_sha256':digest(ROOT/'ORACLE_FREEZE.json'),'observations':records},indent=2)+'\n')
        print('ACCEPTED: 8 exact native and Terminal stdout/stderr/exit oracles',flush=True)
if __name__=='__main__':main()
