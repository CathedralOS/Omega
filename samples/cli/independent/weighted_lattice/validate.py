#!/usr/bin/env python3
"""Check frozen lattice outputs against actual native or captured Terminal runs."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent
ORIGINAL_EXPECTED_SHA256 = 'd2e3447b3b8d0a8dad85e1339aa45867b8f9baf23ef04cbcd511e094c311f131'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--route', choices=['native', 'terminal', 'invalid'], required=True)
    parser.add_argument('--executable', type=Path)
    parser.add_argument('--sponsor', type=Path)
    parser.add_argument('--omega', type=Path)
    options = parser.parse_args()
    data = ROOT / 'data'
    assert hashlib.sha256((data / 'expected.json').read_bytes()).hexdigest() == ORIGINAL_EXPECTED_SHA256
    for line in (data / 'frozen.sha256').read_text().splitlines():
        expected, name = line.split('  ', 1)
        assert hashlib.sha256((data / name).read_bytes()).hexdigest() == expected, name
    fixtures = sorted(data.glob('*.stdin'))
    assert len(fixtures) == 19, 'retain all 19 original inputs'
    output = ROOT / 'build' / 'validation' / options.route
    output.mkdir(parents=True, exist_ok=True)
    records = []
    if options.route == 'invalid':
        if options.omega is None:
            parser.error('--route invalid requires --omega')
        command = [str(options.omega.resolve()), '--check', '--offline', '--target', 'linux_x86_64', str(ROOT / 'invalid' / 'main.omg')]
        observed = subprocess.run(command, input=b'', capture_output=True, timeout=600)
        (output / 'stdout').write_bytes(observed.stdout)
        (output / 'stderr').write_bytes(observed.stderr)
        assert observed.returncode == 1, observed.stderr
        assert b'cannot prove index `9` is within length 9' in observed.stderr, observed.stderr
        records.append({'command': command, 'exit': observed.returncode, 'intended_diagnostic': True})
    elif options.route == 'terminal':
        if options.sponsor is None:
            parser.error('--route terminal requires --sponsor')
        inputs = output / 'inputs'
        inputs.mkdir(exist_ok=True)
        for ordinal, fixture in enumerate(fixtures):
            (inputs / f'{ordinal:02}.stdin').write_bytes(fixture.read_bytes())
        captures = output / 'captures'
        command = [str(options.sponsor.resolve()), str(ROOT), str(inputs), str(captures), '19']
        observed = subprocess.run(command, input=b'', capture_output=True, timeout=600)
        (output / 'sponsor.stdout').write_bytes(observed.stdout)
        (output / 'sponsor.stderr').write_bytes(observed.stderr)
        assert observed.returncode == 0, observed.stderr
        rows = (captures / 'records.tsv').read_text().splitlines()
        assert len(rows) == len(fixtures) == 19
        for ordinal, (fixture, row) in enumerate(zip(fixtures, rows)):
            name, exit_text = row.split('\t')
            assert name == f'{ordinal:02}'
            stdout, stderr = captures / f'{name}.stdout', captures / f'{name}.stderr'
            expected_exit = 64 if fixture.stem.startswith('invalid-') else 0
            assert int(exit_text) == expected_exit, fixture.name
            assert stdout.read_bytes() == fixture.with_suffix('.stdout').read_bytes(), fixture.name
            assert stderr.read_bytes() == b'', fixture.name
            assert (inputs / f'{name}.stdin').read_bytes() == fixture.read_bytes()
            records.append({'case': fixture.stem, 'exit': int(exit_text), 'stdout': str(stdout), 'stderr': str(stderr), 'input_sha256': hashlib.sha256(fixture.read_bytes()).hexdigest(), 'captured_public_terminal': True})
    else:
        if options.executable is None:
            parser.error('--route native requires --executable')
        for fixture in fixtures:
            command = [str(options.executable.resolve())]
            observed = subprocess.run(command, input=fixture.read_bytes(), capture_output=True, timeout=600)
            stdout, stderr = output / f'{fixture.stem}.stdout', output / f'{fixture.stem}.stderr'
            stdout.write_bytes(observed.stdout)
            stderr.write_bytes(observed.stderr)
            expected_exit = 64 if fixture.stem.startswith('invalid-') else 0
            assert observed.returncode == expected_exit, fixture.name
            assert observed.stdout == fixture.with_suffix('.stdout').read_bytes(), fixture.name
            assert observed.stderr == b'', fixture.name
            records.append({'case': fixture.stem, 'exit': observed.returncode, 'stdout': str(stdout), 'stderr': str(stderr), 'input_sha256': hashlib.sha256(fixture.read_bytes()).hexdigest(), 'executable_sha256': hashlib.sha256(options.executable.read_bytes()).hexdigest()})
    (output / 'results.json').write_text(json.dumps({'route': options.route, 'original_expected_sha256': ORIGINAL_EXPECTED_SHA256, 'cases': records}, indent=2) + '\n')
    print(f'{options.route}: {len(records)} original outcomes passed')


if __name__ == '__main__':
    main()
