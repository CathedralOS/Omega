"""Check actual native and captured Terminal numerical observations."""
import argparse
from decimal import Decimal
import hashlib
import json
import os
import signal
from pathlib import Path
import re
import subprocess
import sys
sys.dont_write_bytecode = True
from freeze_reference import oracle
ROOT = Path(__file__).resolve().parent

def check(case, directory):
    assert oracle(case['input']) == {key: case[key] for key in oracle(case['input'])}, 'frozen oracle drift'
    tokens = case['input'].split()
    size, threshold = int(tokens[0]), Decimal(tokens[1])
    for engine in ('native', 'terminal'):
        rows = (directory / f'{engine}.stdout').read_text().splitlines()
        assert len(rows) == 3 * size + 3, (case['name'], engine, rows)
        expected_keys = ['original_pico', 'retained_pico'] * size + ['reconstruction_pico'] * size + ['input_energy_nano', 'coefficient_energy_nano', 'removed_energy_nano']
        assert [row.split('=')[0] for row in rows] == expected_keys
        assert all(re.fullmatch(r'[a-z_]+=-?[0-9]+', row) for row in rows)
        numbers = [Decimal(row.split('=')[1]) for row in rows]
        observed = [numbers[:2*size:2], numbers[1:2*size:2], numbers[2*size:3*size]]
        maximum_error = Decimal(0)
        for values, key in zip(observed, ('original', 'retained', 'reconstruction')):
            errors = [abs(actual / Decimal('1e12') - Decimal(expected)) for actual, expected in zip(values, case[key])]
            maximum_error = max(maximum_error, max(errors))
            assert max(errors) <= Decimal('1e-8'), (case['name'], engine, key, max(errors))
        assert observed[0][0] == observed[1][0], 'coarsest changed'
        energy = Decimal(case['input_energy'])
        measured_input, measured_coefficients, removed = [value / Decimal('1e9') for value in numbers[-3:]]
        scale = max(Decimal(1), energy)
        assert abs(measured_input - energy) <= Decimal('1e-9') * scale
        assert abs(measured_coefficients - measured_input) <= Decimal('1e-9') * scale, 'pre-shrink Parseval'
        assert abs(sum((value / Decimal('1e12')) ** 2 for value in observed[0]) - energy) <= Decimal('1e-9') * scale
        assert abs(removed - Decimal(case['removed_energy'])) <= Decimal('2e-9') * scale
        if threshold == 0:
            assert max(abs(value / Decimal('1e12') - Decimal(sample)) for value, sample in zip(observed[2], tokens[3:])) <= Decimal('1e-8')
        if case['name'] == 'ties':
            assert observed[1][2:4] == [0, 0], 'threshold ties'
        status = (directory / f'{engine}.status').read_text()
        assert status == '0' if engine == 'native' else re.search(r'return_code: 0\s*[,}]', status) is not None
        assert not (directory / f'{engine}.stderr').read_text()
        print(case['name'], engine, 'max_error=', maximum_error)

def invalid_inputs():
    good = '8 0.5 3\n1 -2 3 -4 5 -6 7 -8\n'
    return ['', good.replace('8 0.5', '7 0.5', 1), good.replace('8 0.5', '128 0.5', 1), good.replace('8 0.5', '8.5 0.5', 1), good.replace('0.5', '-0.5', 1), good.replace('0.5', '1001', 1), good.replace('0.5', 'nan', 1), good.replace('0.5', 'inf', 1), good.replace(' 3\n', ' 4\n', 1), good.replace(' 3\n', ' -1\n', 1), good.replace(' 3\n', ' 1.5\n', 1), good.replace(' 3\n', ' 6\n', 1), good.replace('1 -2', 'nan -2'), good.replace('1 -2', 'inf -2'), good.replace('1 -2', '1001 -2'), good.replace('1 -2', '-1001 -2'), good.replace('1 -2', '1e2 -2'), good.replace('1 -2', '1. -2'), good.replace('1 -2', '.5 -2'), good.replace('1 -2', '0.1234567890 -2'), good.replace('1 -2', '- -2'), good.replace('1 -2', '1..0 -2'), good.rstrip()[:-2], good + '9', good + '?', good + ' ' * 4096, ' ' * 4096 + good]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runner', required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    args = parser.parse_args()
    assert os.name == 'posix' and Path('/proc').is_dir(), 'numerical runner qualifies Linux x86-64 only'
    args.evidence.mkdir(parents=True, exist_ok=True)
    cases = json.loads((ROOT / 'fixtures.json').read_text())
    invalid = invalid_inputs()
    # Capture identities and verify independent expectations before launching.
    source_files = sorted(ROOT.rglob('*.omg')) + [ROOT/'fixtures.json', ROOT/'freeze_reference.py', ROOT/'validate.py', ROOT/'validation/src/main.rs', ROOT/'validation/Cargo.toml', Path(args.runner)]
    source_files += sorted((ROOT.parents[3]/'source/library').rglob('*.omg'))
    source_files.append(ROOT/'omega.lock')
    identities = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_files}
    for case in cases:
        expected = oracle(case['input'])
        assert expected == {key: case[key] for key in expected}, 'frozen oracle drift before execution'
    (args.evidence/'preexecution-identities.json').write_text(json.dumps(identities, indent=2)+'\n')
    paths = []
    for name, text in [(case['name'], case['input']) for case in cases] + [(f'invalid{i}', text) for i, text in enumerate(invalid)]:
        directory = args.evidence / name
        directory.mkdir(exist_ok=True)
        path = directory / 'input.txt'
        path.write_text(text)
        paths.append(path)
    command = [args.runner, str(ROOT / 'main.omg'), str(args.evidence), *map(str, paths)]
    # The runner bounds each native child. The containing process group also
    # closes on an outer timeout, including a stalled compilation/preparation.
    runner = subprocess.Popen(command, start_new_session=os.name == 'posix')
    try:
        exit_code = runner.wait(timeout=240)
    except subprocess.TimeoutExpired:
        if os.name == 'posix':
            # Native bounded-process children own separate groups in the same
            # Linux session. Close those groups before killing the runner.
            groups = {runner.pid}
            for entry in Path('/proc').iterdir():
                if not entry.name.isdigit():
                    continue
                try:
                    fields = (entry/'stat').read_text().rsplit(')', 1)[1].split()
                    if int(fields[3]) == runner.pid:
                        groups.add(int(fields[2]))
                except (FileNotFoundError, ProcessLookupError):
                    continue
            for group in groups - {runner.pid}:
                try:
                    os.killpg(group, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            os.killpg(runner.pid, signal.SIGKILL)
        else:
            subprocess.run(['taskkill', '/PID', str(runner.pid), '/T', '/F'], check=True)
        runner.wait()
        raise
    assert exit_code == 0, ('runner failed', exit_code)
    assert identities == {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in source_files}, 'inputs changed during execution'
    for case in cases:
        check(case, args.evidence / case['name'])
    for index in range(len(invalid)):
        for engine in ('native', 'terminal'):
            directory = args.evidence / f'invalid{index}'
            assert (directory / f'{engine}.stdout').read_text() == 'ERROR input or options\n', (index, engine)
            assert not (directory / f'{engine}.stderr').read_text()
            status = (directory / f'{engine}.status').read_text()
            assert status == '10' if engine == 'native' else re.search(r'return_code: 10\s*[,}]', status) is not None, (index, engine, status)
    print(f'{len(cases)} numerical cases and {len(invalid)} refusals checked in both engines')
    (args.evidence / 'summary.json').write_text(json.dumps(dict(valid=len(cases), invalid=len(invalid), engines=['native','terminal'], oracle_sha256=hashlib.sha256((ROOT/'fixtures.json').read_bytes()).hexdigest()), indent=2)+'\n')

if __name__ == '__main__':
    main()
