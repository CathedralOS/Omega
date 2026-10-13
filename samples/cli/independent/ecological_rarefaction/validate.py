"""Serial qualification of actual native and retained Terminal numerical streams.

The runner reads the frozen oracle; it never invokes reference.py or rewrites
expectations. Every positive row checks exact bin weights, both moment rational
pairs, normalization, and independent inclusion/covariance moments at 1e-10.
"""
import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import time
ROOT = Path(__file__).resolve().parent

def verify(output, case):
    if case['exit']:
        assert output == case['stdout'].encode(), output
        return
    lines = iter(output.decode('ascii').splitlines())
    assert next(lines) == 'ecological_rarefaction'
    assert next(lines) == f"species={len(case['counts'])}"
    assert next(lines) == f"individuals={sum(case['counts'])}"
    for row in case['rows']:
        assert next(lines) == f"depth={row['depth']}"
        line = next(lines)
        assert re.fullmatch(r'denominator=\d+', line), line
        denominator = int(line.split('=')[1])
        assert denominator == row['denominator']
        weights = []
        for richness, expected in enumerate(row['weights']):
            match = re.fullmatch(r'bin=(\d+) numerator=(\d+)', next(lines))
            assert match and int(match[1]) == richness and int(match[2]) == expected
            weights.append(int(match[2]))
        values = {}
        for key in ('expectation_numerator', 'expectation_denominator', 'variance_numerator', 'variance_denominator'):
            line = next(lines)
            assert re.fullmatch(key+r'=\d+', line), line
            values[key] = int(line.split('=')[1])
        assert values == dict(expectation_numerator=row['expectation_numerator'],
                              expectation_denominator=denominator,
                              variance_numerator=row['variance_numerator'],
                              variance_denominator=row['variance_denominator'])
        probabilities = [Fraction(weight, denominator) for weight in weights]
        expectation = Fraction(values['expectation_numerator'], denominator)
        variance = Fraction(values['variance_numerator'], values['variance_denominator'])
        assert sum(probabilities) == 1
        assert sum(r*p for r,p in enumerate(probabilities)) == expectation
        assert sum(r*r*p for r,p in enumerate(probabilities))-expectation**2 == variance
        assert abs(float(sum(probabilities))-1) <= 1e-10
        assert abs(float(expectation)-float(Fraction(row['expectation']))) <= 1e-10
        assert abs(float(variance)-float(Fraction(row['variance']))) <= 1e-10
    assert list(lines) == []

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', required=True)
    parser.add_argument('--route', choices=('native','terminal','captured','source-invalid'), required=True)
    parser.add_argument('--captures', type=Path, help='public Terminal SDK capture directory')
    parser.add_argument('--cases', help='comma separated subset; default all frozen cases')
    options = parser.parse_args()
    binary = str(Path(options.omega).resolve())
    output = ROOT/'build/validation'/options.route
    output.mkdir(parents=True, exist_ok=True)
    frozen = ROOT/'data/oracle.json'
    assert hashlib.sha256(frozen.read_bytes()).hexdigest() == (ROOT/'ORACLE_SHA256').read_text().strip()
    oracle = json.loads(frozen.read_text())
    receipts = []
    def run(label, command, data=b''):
        started = time.monotonic()
        result = subprocess.run(command, input=data, capture_output=True, timeout=300, cwd=ROOT)
        (output/f'{label}.stdout').write_bytes(result.stdout)
        (output/f'{label}.stderr').write_bytes(result.stderr)
        receipts.append(dict(label=label, command=command, exit=result.returncode,
                             seconds=time.monotonic()-started,
                             stdout_sha256=hashlib.sha256(result.stdout).hexdigest()))
        (output/'receipts.json').write_text(json.dumps(receipts,indent=2)+'\n')
        print(label, result.returncode, f'{receipts[-1]["seconds"]:.2f}s', flush=True)
        return result
    if options.route == 'source-invalid':
        result = run('richness_bound', [binary,'--check',str(ROOT/'controls/richness_bound/main.omg')])
        assert result.returncode == 1
        fragments = (ROOT/'controls/richness_bound/expected.txt').read_text().splitlines()
        assert all(fragment.encode() in result.stderr for fragment in fragments), result.stderr
        assert b'parse error' not in result.stderr and b'package acceptance' not in result.stderr
        return
    if options.route == 'captured':
        assert options.captures
        for name, case in oracle['cases'].items():
            data = (ROOT/'data'/f'{name}.stdin').read_bytes()
            assert hashlib.sha256(data).hexdigest() == case['input_sha256']
            assert (options.captures/f'terminal-{name}.stdin').read_bytes() == data
            assert int((options.captures/f'terminal-{name}.exit').read_text()) == case['exit']
            verify((options.captures/f'terminal-{name}.stdout').read_bytes(), case)
            assert not (options.captures/f'terminal-{name}.stderr').read_bytes()
            print(name, 'captured Terminal values PASS', flush=True)
        return
    native = None
    if options.route == 'native':
        directory = ROOT/'build/native'
        directory.mkdir(parents=True,exist_ok=True)
        result = run('compile',[binary,'--target','linux_x86_64','--build-dir',str(directory),str(ROOT/'main.omg')])
        assert result.returncode == 0, result.stderr.decode()
        native = directory/'omega-program'
        assert native.is_file(), list(directory.iterdir())
    selected = options.cases.split(',') if options.cases else list(oracle['cases'])
    for name in selected:
        case = oracle['cases'][name]
        data = (ROOT/'data'/f'{name}.stdin').read_bytes()
        assert hashlib.sha256(data).hexdigest() == case['input_sha256']
        command = [str(native)] if native else [binary,'run','--both','--keep',str(ROOT/'main.omg')]
        result = run(name,command,data)
        assert result.returncode == case['exit'], result.stderr.decode()
        verify(result.stdout,case)
        if native:
            assert not result.stderr, result.stderr
        else:
            assert b'DECLINED' not in result.stderr and b'DIVERGENCE' not in result.stderr, result.stderr
            assert f"interp exit: {case['exit']}".encode() in result.stderr, result.stderr
            match = re.search(rb'build dir kept: (.+)', result.stderr)
            assert match, result.stderr
            retained = Path(match[1].decode())
            terminal = (retained/'terminal.stdout').read_bytes()
            terminal_error = (retained/'terminal.stderr').read_bytes()
            (output/f'{name}.terminal.stdout').write_bytes(terminal)
            (output/f'{name}.terminal.stderr').write_bytes(terminal_error)
            verify(terminal,case)
            assert not terminal_error
            receipts[-1]['terminal_stdout_sha256'] = hashlib.sha256(terminal).hexdigest()
            # Only this invocation's successful disposable retained directory.
            shutil.rmtree(retained)
            (output/'receipts.json').write_text(json.dumps(receipts,indent=2)+'\n')
    print(f'{len(selected)} {options.route} cases verified against frozen exact oracle',flush=True)
if __name__ == '__main__': main()
