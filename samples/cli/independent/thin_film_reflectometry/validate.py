"""Compare actual native and captured Terminal values to frozen independent rows."""
import argparse
import hashlib
import json
import math
import re
import subprocess
from pathlib import Path
ROOT = Path(__file__).resolve().parent
FIELDS = ['r_real_pico', 'r_imaginary_pico', 't_real_pico', 't_imaginary_pico', 'R_pico', 'T_pico']


def status_is(status, engine, expected):
    if engine == 'native':
        return status == str(expected)
    matched = re.search(r', return_code: (-?\d+),', status)
    return status.startswith('ExternalCompletion(TerminalExternalCompletion { ') and matched is not None and int(matched.group(1)) == expected


def check(case, directory, engine):
    lines = (directory / f'{engine}.stdout').read_text().splitlines()
    assert len(lines) == len(case['outputs']) * 7 + 2, (engine, lines)
    values = []
    maximum = 0
    for position, expected in enumerate(case['outputs']):
        row = lines[position*7:position*7+7]
        assert row[0].startswith('wavelength_pico_nm=')
        assert abs(int(row[0].split('=')[1])/1e12 - case['wavelengths'][position]) <= 1e-8
        measured = []
        for field, line, reference in zip(FIELDS, row[1:], expected):
            key, value = line.split('=')
            assert key == field
            numeric = int(value)/1e12
            assert math.isfinite(numeric) and abs(numeric-reference) <= 1e-8, (case['name'], engine, field, numeric, reference)
            maximum = max(maximum, abs(numeric-reference))
            measured.append(numeric)
        assert abs(measured[-2] + measured[-1] - 1) <= 1e-8
        # Independent consistency of emitted fields and powers.
        assert abs(measured[-2] - abs(complex(*measured[:2]))**2) <= 1e-8
        assert abs(measured[-1] - case['substrate']/case['ambient'] * abs(complex(*measured[2:4]))**2) <= 1e-8
        values.append(measured)
    minimum = min(range(len(case['outputs'])), key=lambda position: case['outputs'][position][-2])
    assert lines[-2].startswith('minimum_R_pico=') and lines[-1].startswith('minimum_wavelength_pico_nm=')
    assert abs(int(lines[-2].split('=')[1])/1e12 - case['outputs'][minimum][-2]) <= 1e-8
    assert abs(int(lines[-1].split('=')[1])/1e12 - case['wavelengths'][minimum]) <= 1e-8
    status = (directory / f'{engine}.status').read_text()
    assert status_is(status, engine, 0), status
    return dict(case=case['name'], engine=engine, rows=len(values), maximum_error=maximum, values=values)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--runner', required=True)
    parser.add_argument('--evidence', required=True, type=Path)
    arguments = parser.parse_args()
    arguments.evidence.mkdir(parents=True, exist_ok=True)
    cases = json.loads((ROOT/'fixtures.json').read_text())
    valid = '1 1.5 0 1 550\n'
    invalid = ['', 'nan 1 0 1 550', '1 inf 0 1 550', '-1 1 0 1 550', '0.999 1 0 1 550',
               '4.001 1 0 1 550', '1 0.99 0 1 550', '1 4.01 0 1 550', '1 1 -1 1 550',
               '1 1 9 1 550', '1 1 0.5 1 550', '1 1 0 0', '1 1 0 33', '1 1 0 1.5 550',
               '1 1 1 1 0.99 50 550', '1 1 1 1 4.01 50 550', '1 1 1 1 2 -0.01 550',
               '1 1 1 1 2 2000.01 550', '1 1 0 1 199.99', '1 1 0 1 2000.01',
               valid+'1', valid+'?', valid+' '*4096, '1 1 0 1', '1 1 1 1 2 50',
               '1e0 1 0 1 550', '+1 1 0 1 550', '1. 1 0 1 550', '.1 1 0 1 550',
               '1..0 1 0 1 550', '0000000000001 1 0 1 550', '1 1 0 1 -inf', valid.rstrip().ljust(4096)]
    inputs = []
    for name, content in [(case['name'], case['input']) for case in cases] + [(f'invalid{position}', content) for position, content in enumerate(invalid)]:
        directory = arguments.evidence/name
        directory.mkdir(exist_ok=True)
        path = directory/'input.txt'
        path.write_text(content)
        inputs.append(path)
    subprocess.run([arguments.runner, str(ROOT/'main.omg'), str(arguments.evidence), *map(str, inputs)], check=True, timeout=240)
    observations = [check(case, arguments.evidence/case['name'], engine) for case in cases for engine in ('native', 'terminal')]
    for position in range(len(invalid)):
        for engine in ('native', 'terminal'):
            directory = arguments.evidence/f'invalid{position}'
            assert (directory/f'{engine}.stdout').read_text() == 'ERROR input or domain\n'
            status = (directory/f'{engine}.status').read_text()
            assert status_is(status, engine, 10), status
    for engine in ('native', 'terminal'):
        forward = next(item['values'] for item in observations if item['case']=='forward' and item['engine']==engine)
        reverse = next(item['values'] for item in observations if item['case']=='reverse' and item['engine']==engine)
        for left,right in zip(forward,reverse):
            assert abs(left[-2]-right[-2]) <= 1e-8 and abs(left[-1]-right[-1]) <= 1e-8
            assert abs(complex(*left[2:4])*3.1/1.2 - complex(*right[2:4])) <= 1e-8
    record = dict(oracle_sha256=hashlib.sha256((ROOT/'fixtures.json').read_bytes()).hexdigest(), observations=observations, invalid_cases=len(invalid))
    (arguments.evidence/'numerical.json').write_text(json.dumps(record, indent=2)+'\n')
    print(f"{len(observations)} numerical observations, {len(invalid)} refusals per engine; maximum error {max(item['maximum_error'] for item in observations):.3g}")

if __name__ == '__main__':
    main()
