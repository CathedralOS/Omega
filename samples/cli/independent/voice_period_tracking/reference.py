#!/usr/bin/env python3
"""Independent exhaustive integer oracle for synthetic sample models.

Freeze writes expected bytes before Omega execution. Verification never updates
oracles. Fraction computes normalized costs independently of Omega's streaming
cross-product comparison, using complete materialized pair differences.
"""
from fractions import Fraction
from pathlib import Path
import argparse
import hashlib
import json
import subprocess

ROOT = Path(__file__).resolve().parent

def oracle(payload):
    output = []
    windows = []
    current = []
    previous = 0
    uncertain = None
    def frame(values):
        nonlocal previous, uncertain
        index = len(windows)
        windows.append(values)
        output.append(f'FRAME {index} {len(values)}')
        chosen, status = 0, 3
        if len(values) >= 12:
            scores = {lag: (sum((left-right)**2 for left, right in zip(values, values[lag:])), len(values)-lag) for lag in range(2, 9)}
            for lag, (total, count) in scores.items():
                output.append(f'LAG {lag} {total} {count}')
            costs = {lag: Fraction(*score) for lag, score in scores.items()}
            minimum = min(costs.values())
            winners = sorted(lag for lag, cost in costs.items() if cost == minimum)
            if len(set(values)) == 1:
                status = 0
            elif minimum > 1:
                status = 4
            else:
                chosen = winners[0]
                status = 2 if len(winners) > 1 else 1
        output.append(f'TRACK {index} {chosen} {8000//chosen if chosen else 0} {status}')
        if previous and chosen and previous != chosen:
            output.append(f'CHANGE {index} {previous} {chosen}')
        previous = chosen
        if status == 1 and uncertain is not None:
            output.append(f'SEGMENT {uncertain} {index}')
            uncertain = None
        if status != 1 and uncertain is None:
            uncertain = index
    def result(code):
        return ('\n'.join(output) + '\n').encode(), code
    for byte in payload:
        if byte == 10:
            if not current or len(windows) >= 16:
                output.append('ERROR input')
                return result(64)
            frame(current)
            current = []
        elif not 48 <= byte <= 57 or len(current) >= 32 or len(windows) >= 16:
            output.append('ERROR input')
            return result(64)
        else:
            current.append(byte-48)
    if current:
        frame(current)
    if not windows:
        output.append('ERROR input')
        return result(64)
    if uncertain is not None:
        output.append(f'SEGMENT {uncertain} {len(windows)}')
    return result(0)

def fixtures():
    # Explicit synthetic PCM models, not recordings or evidence of human voice.
    yield 'trajectory', ('04740'*6+'\n'+'0259741'*4+'\n'+'0950'*8+'\n'+'5'*24+'\n'+'012345678'+'\n'+'25652'*6+'\n').encode()
    yield 'harmonics', ('09'*16+'\n'+'04740'*3+'0259741'*2+'\n'+'01928374650192837465019283746501'+'\n').encode()
    yield 'boundary', ('04740'*2+'04'+'\n'+'0259741'*4+'0259').encode()
    yield 'invalid_empty', b''
    yield 'invalid_blank', b'\n'
    yield 'invalid_character', b'012x'
    yield 'invalid_cr', b'012\r\n'
    yield 'invalid_capacity', b'0'*33+b'\n'
    yield 'invalid_records', b'0\n'*17

def freeze():
    manifest = {'model': 'PCM digits 0..9; sample rate 8000; independent windows', 'tolerance': 0, 'score': 'sum squared differences / pair count; exhaustive lags 2..8', 'cases': []}
    for name, payload in fixtures():
        expected, code = oracle(payload)
        for suffix, content in [('input', payload), ('stdout', expected), ('exit', str(code).encode()+b'\n')]:
            path = ROOT/'data'/f'{name}.{suffix}'
            path.write_bytes(content)
        manifest['cases'].append({'name': name, 'exit': code, 'input_sha256': hashlib.sha256(payload).hexdigest(), 'stdout_sha256': hashlib.sha256(expected).hexdigest()})
    (ROOT/'data'/'oracle.json').write_text(json.dumps(manifest, indent=2)+'\n')
    print('froze nine cases; tolerance 0')

def verify(binary, both):
    evidence = []
    for case in json.loads((ROOT/'data'/'oracle.json').read_text())['cases']:
        name = case['name']
        payload = (ROOT/'data'/f'{name}.input').read_bytes()
        expected = (ROOT/'data'/f'{name}.stdout').read_bytes()
        assert hashlib.sha256(payload).hexdigest() == case['input_sha256']
        assert hashlib.sha256(expected).hexdigest() == case['stdout_sha256']
        command = [str(Path(binary).resolve()), 'run'] + (['--both'] if both else []) + ['--keep', str(ROOT/'main.omg')]
        run = subprocess.run(command, input=payload, capture_output=True, timeout=180)
        directory = ROOT/'build'/('both' if both else 'native')
        directory.mkdir(parents=True, exist_ok=True)
        (directory/f'{name}.stdout').write_bytes(run.stdout)
        (directory/f'{name}.stderr').write_bytes(run.stderr)
        row = {'case': name, 'command': command, 'exit': run.returncode, 'expected_exit': case['exit'], 'stdout_match': run.stdout == expected, 'terminal_exit_match': (f"interp exit: {case['exit']}\n".encode() in run.stderr and b'DECLINED' not in run.stderr) if both else None, 'stderr': run.stderr.decode(errors='replace')}
        evidence.append(row)
        (directory/'results.json').write_text(json.dumps(evidence, indent=2)+'\n')
        print(name, run.returncode, 'match' if row['stdout_match'] else 'DIFF', flush=True)
        if run.returncode != case['exit'] or not row['stdout_match'] or (both and not row['terminal_exit_match']):
            raise SystemExit('validation failed; preserved logs')
    print('all nine exact byte and exit checks passed')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--freeze', action='store_true')
    parser.add_argument('--omega')
    parser.add_argument('--both', action='store_true')
    options = parser.parse_args()
    if options.freeze:
        freeze()
    elif options.omega:
        verify(options.omega, options.both)
    else:
        parser.error('select --freeze or --omega')
