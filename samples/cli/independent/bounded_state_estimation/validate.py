"""Sequential application validation against a committed, immutable oracle.

Run after package review: python3 validate.py --omega /absolute/path/omega
Artifacts are retained in build/validation. No expected values are generated here.
"""
import argparse
from fractions import Fraction
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parent

def validate_trace(output, case, oracle):
    observed = [line.split() for line in output.splitlines() if line.startswith('TRACE ')]
    assert len(observed) == len(case['trace']), (len(observed), len(case['trace']))
    for actual, expected in zip(observed, case['trace']):
        assert len(actual) == 11, actual
        assert int(actual[1]) == expected['step'] and actual[-1] == expected['policy'], actual
        numbers = [int(value)/1e6 for value in actual[2:-1]]
        for number, exact in zip(numbers, expected['exact']):
            reference = float(Fraction(exact))
            assert math.isclose(number, reference, abs_tol=oracle['absolute_tolerance'], rel_tol=oracle['relative_tolerance']), (actual, number, exact)
        a,b,c,d = numbers[2:6]
        tolerance = oracle['absolute_tolerance']
        assert abs(b-c) <= 2*tolerance and a >= -tolerance and d >= -tolerance
        determinant_error = tolerance*(abs(a)+abs(b)+abs(c)+abs(d))+2*tolerance*tolerance
        assert a*d-b*c >= -determinant_error
    if case['exit'] == 0:
        assert 'DONE covariance symmetric and positive semidefinite' in output
    else:
        assert 'ERROR '+case['error'] in output

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', required=True)
    parser.add_argument('--native-only', action='store_true')
    options = parser.parse_args()
    binary = Path(options.omega).resolve()
    artifacts = ROOT/'build/validation'
    artifacts.mkdir(parents=True, exist_ok=True)
    oracle_path = ROOT/'expected.json'
    oracle = json.loads(oracle_path.read_text())
    records = []
    def execute(name, command, data=b''):
        start = time.monotonic()
        result = subprocess.run(command, input=data, capture_output=True, cwd=ROOT, timeout=300)
        (artifacts/(name+'.stdout')).write_bytes(result.stdout)
        (artifacts/(name+'.stderr')).write_bytes(result.stderr)
        records.append({'name': name, 'command': [str(x) for x in command], 'exit': result.returncode, 'seconds': time.monotonic()-start})
        print(name, result.returncode, flush=True)
        return result
    try:
        checked = execute('check', [binary, '--check', '--target', 'linux_x86_64', ROOT/'main.omg'])
        assert checked.returncode == 0, checked.stderr.decode(errors='replace')
        invalid = execute('intended_invalid', [binary, '--check', ROOT/'intended_invalid/invalid_index.omg'])
        invalid_text = (invalid.stdout+invalid.stderr).decode(errors='replace')
        assert invalid.returncode == 1 and (ROOT/'intended_invalid/expected.txt').read_text().strip() in invalid_text, invalid_text
        product = execute('native_build', [binary, '--target', 'linux_x86_64',
                          '--build-dir', ROOT/'build/native_validation',
                          '--report-file', artifacts/'native-product.txt', ROOT/'main.omg'])
        assert product.returncode == 0, product.stderr.decode(errors='replace')
        published = [line.removeprefix('published native output to ') for line in
                     (artifacts/'native-product.txt').read_text().splitlines()
                     if line.startswith('published native output to ')]
        assert len(published) == 1, published
        executable = Path(published[0])
        if not executable.is_absolute():
            executable = ROOT/executable
        assert executable.is_file(), executable
        for name, case in oracle['cases'].items():
            fixture = (ROOT/'data'/f'{name}.txt').read_bytes()
            assert hashlib.sha256(fixture).hexdigest() == case['input_sha256'], name
            result = execute('native_'+name, [executable], fixture)
            assert result.returncode == case['exit'], (name, case['exit'], result.stderr.decode(errors='replace'))
            validate_trace(result.stdout.decode(), case, oracle)
            if not options.native_only:
                comparison = execute('terminal_'+name, [binary, 'run', '--both', '--keep', ROOT/'main.omg'], fixture)
                assert comparison.returncode == case['exit'], (name, comparison.stderr.decode(errors='replace'))
                validate_trace(comparison.stdout.decode(), case, oracle)
                # The documented CLI compares termination, not trace bytes. A
                # decline is retained as a blocker, never counted as agreement.
                text = comparison.stderr.decode(errors='replace')
                assert 'DECLINED' not in text and f"interp exit: {case['exit']}" in text, text
        print('All frozen trajectory and rejection checks matched.', flush=True)
    finally:
        receipt = {'host': os.uname().sysname+' '+os.uname().machine, 'binary': str(binary),
                   'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                   'oracle_sha256': hashlib.sha256(oracle_path.read_bytes()).hexdigest(),
                   'commands': records}
        (artifacts/'receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')

if __name__ == '__main__':
    main()
