"""Run exact authored oracles sequentially; never rebuild or rewrite expectations."""
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
    arguments.add_argument('--mode', choices=['native','both','terminal'], required=True)
    arguments.add_argument('--case', action='append')
    arguments.add_argument('--terminal-harness', type=Path)
    arguments.add_argument('--native-executable', type=Path)
    options = arguments.parse_args()
    binary = options.omega.resolve()
    frozen = json.loads((ROOT/'expected/frozen.json').read_text())
    if options.case:
        assert set(options.case) <= {case['name'] for case in frozen['cases']}, 'unknown fixture selected'
    results = ROOT/'build/validation'/options.mode
    results.mkdir(parents=True,exist_ok=True)
    records=[]
    if options.mode == 'terminal':
        assert options.terminal_harness and not options.case, 'Terminal validation requires a harness and runs every frozen case'
        assert (ROOT/'expected/cases.tsv').read_text() == ''.join(f"{case['name']}\t{case['exit']}\n" for case in frozen['cases'])
        for case in frozen['cases']:
            assert digest((ROOT/'data'/f"{case['name']}.txt").read_bytes()) == case['input_sha256']
            assert digest((ROOT/'expected'/f"{case['name']}.stdout.txt").read_bytes()) == case['stdout_sha256']
        command = [str(options.terminal_harness.resolve()), str(ROOT)]
        completed = subprocess.run(command, capture_output=True, timeout=600)
        (results/'harness.stdout.log').write_bytes(completed.stdout)
        (results/'harness.stderr.log').write_bytes(completed.stderr)
        print(completed.stdout.decode(), end='', flush=True)
        assert completed.returncode == 0, completed.stderr.decode(errors='replace')
    for case in frozen['cases']:
        name=case['name']
        if options.case and name not in options.case:
            continue
        input_bytes=(ROOT/'data'/f'{name}.txt').read_bytes()
        expected=(ROOT/'expected'/f'{name}.stdout.txt').read_bytes()
        assert digest(input_bytes)==case['input_sha256'], f'input altered: {name}'
        assert digest(expected)==case['stdout_sha256'], f'oracle altered: {name}'
        command=[str(binary),'run']+(['--both'] if options.mode=='both' else [])+[str(ROOT/'main.omg')]
        if options.mode == 'terminal':
            command = [str(options.terminal_harness.resolve()), str(ROOT)]
            completed = subprocess.CompletedProcess(command, int((results/f'{name}.exit.txt').read_text()),
                (results/f'{name}.stdout.txt').read_bytes(), (results/f'{name}.stderr.txt').read_bytes())
        else:
            if options.native_executable:
                assert options.mode == 'native', 'direct executable is native validation only'
                command = [str(options.native_executable.resolve())]
            completed=subprocess.run(command,input=input_bytes,capture_output=True,timeout=180)
        (results/f'{name}.stdout.txt').write_bytes(completed.stdout)
        (results/f'{name}.stderr.txt').write_bytes(completed.stderr)
        assert completed.returncode==case['exit'], (name,completed.returncode,completed.stderr.decode(errors='replace'))
        if options.mode == 'terminal' or options.native_executable:
            assert completed.stderr == b'', f'{name}: unexpected program stderr'
        assert completed.stdout==expected, f'{name}: stdout mismatch; see {results}'
        if options.mode=='both':
            assert b'DECLINED' not in completed.stderr, f'{name}: comparison declined'
            assert f"interp exit: {case['exit']}\n".encode() in completed.stderr, f'{name}: no affirmative Terminal agreement'
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
    if options.terminal_harness:
        receipt['terminal_harness_sha256'] = digest(options.terminal_harness.read_bytes())
    (results/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(f'{len(records)} cases verified')

if __name__=='__main__':
    main()
