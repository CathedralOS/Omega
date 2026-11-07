"""Run exact authored oracles; never rebuild or rewrite expectations.

`--mode both` compiles once with `omega run --both --input ...`, which runs
every case natively and through the Terminal reference and writes each
engine's streams; both must match the oracle.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent

def digest(data):
    return hashlib.sha256(data).hexdigest()

def main():
    arguments = argparse.ArgumentParser()
    arguments.add_argument('--omega', required=True, type=Path)
    arguments.add_argument('--mode', choices=['native','both'], required=True)
    arguments.add_argument('--case', action='append')
    arguments.add_argument('--native-executable', type=Path)
    options = arguments.parse_args()
    binary = options.omega.resolve()
    frozen = json.loads((ROOT/'expected/frozen.json').read_text())
    if options.case:
        assert set(options.case) <= {case['name'] for case in frozen['cases']}, 'unknown fixture selected'
    results = ROOT/'build/validation'/options.mode
    results.mkdir(parents=True,exist_ok=True)
    records=[]
    statuses = {}
    if options.mode == 'both':
        selected = [case['name'] for case in frozen['cases'] if not options.case or case['name'] in options.case]
        cases = results/'cases'
        command = [str(binary),'run','--both'] + [part for name in selected for part in ('--input', str(ROOT/'data'/f'{name}.txt'))] + ['--outputs', str(cases), str(ROOT/'main.omg')]
        completed = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True, timeout=1800)
        (results/'cli.stderr.log').write_bytes(completed.stderr)
        assert completed.returncode == 0, completed.stderr.decode(errors='replace')
        statuses = {row[0]: row for row in (line.split('\t') for line in (cases/'results.tsv').read_text().splitlines()[1:])}
    for case in frozen['cases']:
        name=case['name']
        if options.case and name not in options.case:
            continue
        input_bytes=(ROOT/'data'/f'{name}.txt').read_bytes()
        expected=(ROOT/'expected'/f'{name}.stdout.txt').read_bytes()
        assert digest(input_bytes)==case['input_sha256'], f'input altered: {name}'
        assert digest(expected)==case['stdout_sha256'], f'oracle altered: {name}'
        command=[str(binary),'run',str(ROOT/'main.omg')]
        if options.mode == 'both':
            _, native, verdict, interpreter = statuses[name]
            assert verdict == 'agrees' and native == interpreter, (name, statuses[name])
            cases = results/'cases'
            completed = subprocess.CompletedProcess(['omega run --both --input'], int(native),
                (cases/f'{name}.stdout').read_bytes(), (cases/f'{name}.stderr').read_bytes())
            assert (cases/f'{name}.terminal.stdout').read_bytes() == completed.stdout, f'{name}: Terminal stdout differs'
            assert (cases/f'{name}.terminal.stderr').read_bytes() == b'', f'{name}: Terminal stderr'
        else:
            if options.native_executable:
                assert options.mode == 'native', 'direct executable is native validation only'
                command = [str(options.native_executable.resolve())]
            completed=subprocess.run(command,input=input_bytes,capture_output=True,timeout=180)
        (results/f'{name}.stdout.txt').write_bytes(completed.stdout)
        (results/f'{name}.stderr.txt').write_bytes(completed.stderr)
        assert completed.returncode==case['exit'], (name,completed.returncode,completed.stderr.decode(errors='replace'))
        if options.mode == 'both' or options.native_executable:
            assert completed.stderr == b'', f'{name}: unexpected program stderr'
        assert completed.stdout==expected, f'{name}: stdout mismatch; see {results}'
        if case['exit']==0:
            lines=completed.stdout.splitlines(keepends=True)
            header_end=4
            height=int(lines[2].split()[1])
            pgm=b''.join(lines[:header_end+height]); report=b''.join(lines[header_end+height:])
            assert pgm==(ROOT/'expected'/f'{name}.pgm').read_bytes()
            assert report==(ROOT/'expected'/f'{name}.regions.txt').read_bytes()
            (results/f'{name}.pgm').write_bytes(pgm)
            (results/f'{name}.regions.txt').write_bytes(report)
        records.append({'case':name,'exit':completed.returncode,'stdout_sha256':digest(completed.stdout),'command':command})
        print(f'{options.mode}: {name} exact exit/stdout verified',flush=True)
    receipt={'binary':str(binary),'binary_sha256':digest(binary.read_bytes()),'oracle_sha256':digest((ROOT/'expected/frozen.json').read_bytes()),'mode':options.mode,'results':records}
    if options.native_executable:
        receipt['native_executable_sha256'] = digest(options.native_executable.read_bytes())
    (results/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(f'{len(records)} cases verified')

if __name__=='__main__':
    main()
