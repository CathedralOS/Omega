#!/usr/bin/env python3
"""Serial compiler/native/Terminal acceptance; never edits frozen expectations."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time
import sys
sys.dont_write_bytecode = True
from reference import analyze

ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[3]

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', required=True)
    parser.add_argument('--compiler-source-commit', required=True)
    parser.add_argument('--case', action='append', help='fixture stem; default all')
    options = parser.parse_args()
    binary = Path(options.omega).resolve()
    output_directory = ROOT / 'build' / 'validation'
    output_directory.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, OMEGA_LIBRARY_ROOT=str(REPOSITORY/'source/library'))
    frozen = json.loads((ROOT/'oracles.sha256.json').read_text())
    for name, expected in frozen.items():
        assert digest(ROOT/name) == expected, f'frozen oracle changed: {name}'
    fixtures = sorted((ROOT/'data').glob('*.stdin'))
    if options.case:
        fixtures = [file for file in fixtures if file.stem in options.case]
        assert len(fixtures) == len(set(options.case)), 'unknown requested fixture'
    for file in fixtures:
        stdout, code = analyze(file.read_text())
        assert stdout.encode() == file.with_suffix('.stdout').read_bytes(), f'reference stdout: {file.stem}'
        assert code == int(file.with_suffix('.exit').read_text()), f'reference exit: {file.stem}'
    report = {'compiler_source_commit':options.compiler_source_commit,
              'rust_toolchain':(REPOSITORY/'rust-toolchain.toml').read_text(),
              'root_owner_uid':os.stat('/').st_uid,
              'source_head': subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPOSITORY,text=True).strip(),
              'binary': str(binary), 'binary_sha256':digest(binary),
              'target':'linux_x86_64', 'host':platform.platform(), 'cpu_count':os.cpu_count(),
              'uid':os.getuid() if hasattr(os,'getuid') else None,
              'library':str(REPOSITORY/'source/library'), 'numerical_tolerance':0,
              'source_sha256':{str(file.relative_to(ROOT)):digest(file) for file in sorted(ROOT.rglob('*'))
                               if file.is_file() and 'build' not in file.relative_to(ROOT).parts
                               and '__pycache__' not in file.parts}, 'commands':[]}
    def save(): (ROOT/'build'/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
    def execute(name, arguments, input_bytes=b''):
        started=time.monotonic()
        command=[str(binary), *arguments]
        print('RUN', name, flush=True)
        result=subprocess.run(command,input=input_bytes,capture_output=True,env=environment,cwd=REPOSITORY,timeout=600)
        (output_directory/(name+'.stdout')).write_bytes(result.stdout)
        (output_directory/(name+'.stderr')).write_bytes(result.stderr)
        row={'name':name,'command':command,'exit':result.returncode,'elapsed_seconds':time.monotonic()-started,
             'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),
             'stdout_artifact':str((output_directory/(name+'.stdout')).relative_to(ROOT)),
             'stderr_artifact':str((output_directory/(name+'.stderr')).relative_to(ROOT))}
        report['commands'].append(row); save()
        return result,row
    result,row=execute('check',['--check',str(ROOT/'main.omg'),'--build-dir',str(ROOT/'build'/'check')])
    assert result.returncode == 0, result.stderr.decode(errors='replace')
    row['accepted']=True;save()
    for file in fixtures:
        result,row=execute(file.stem,['run','--both','--keep',str(ROOT/'main.omg')],file.read_bytes())
        expected_code=int(file.with_suffix('.exit').read_text())
        row['stdin']=str(file.relative_to(ROOT)); row['expected_exit']=expected_code
        row['stdout_matches']=result.stdout==file.with_suffix('.stdout').read_bytes()
        row['terminal_exit_matches']=f'interp exit: {expected_code}\n'.encode() in result.stderr
        row['accepted']=result.returncode==expected_code and row['stdout_matches'] and row['terminal_exit_matches']
        # The documented CLI comparison checks termination; it does not expose
        # Terminal buffered stdout. Native stdout is independently checked here.
        row['terminal_comparison_scope']='exit; native stdout checked against frozen oracle'
        save()
        assert row['accepted'], f'{file.stem}: exit={result.returncode}; stdout={row["stdout_matches"]}; {result.stderr.decode(errors="replace")}'
        print('PASS',file.stem,flush=True)
    result,row=execute('intended_invalid',['--check',str(ROOT/'invalid'/'unchecked_timestamp.omg'),'--build-dir',str(ROOT/'build'/'invalid')])
    diagnostic=(ROOT/'invalid'/'expected_diagnostic.txt').read_text().strip().encode()
    row['accepted']=result.returncode==1 and diagnostic in result.stderr
    save()
    assert row['accepted'], result.stderr.decode(errors='replace')
    report['accepted']=True;save()
    print(f'accepted check, {len(fixtures)} native/Terminal runs and intended rejection',flush=True)

if __name__=='__main__': main()
