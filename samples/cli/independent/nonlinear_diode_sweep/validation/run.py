"""Serial check/native/Terminal routes. No package acceptance is performed."""
import argparse
import hashlib
import json
import subprocess
import sys
from decimal import localcontext
from pathlib import Path

from check_output import check

ROOT = Path(__file__).resolve().parent.parent


def run(binary, target, route, cases, native_executable=None):
    destination = ROOT / 'build/validation' / route
    destination.mkdir(parents=True, exist_ok=True)
    records = []
    if route in ('check', 'invalid'):
        source = ROOT / ('main.omg' if route == 'check' else 'controls/intended_invalid/main.omg')
        command = [binary, '--check', '--target', target, str(source)]
        result = subprocess.run(command, capture_output=True, timeout=300)
        (destination / 'stdout.txt').write_bytes(result.stdout)
        (destination / 'stderr.txt').write_bytes(result.stderr)
        record = dict(command=command, exit=result.returncode)
        records.append(record)
        if route == 'check':
            assert result.returncode == 0, result.stderr.decode(errors='replace')
        else:
            diagnostics = result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')
            assert result.returncode == 1 and 'index' in diagnostics.lower(), diagnostics
            assert 'cannot prove' in diagnostics.lower() or 'out of bounds' in diagnostics.lower(), diagnostics
            assert 'package acceptance' not in diagnostics.lower()
    else:
        oracle = json.loads((ROOT / 'validation/oracle.json').read_text())
        for name in cases or oracle['cases']:
            command = [binary, 'run', '--keep']
            if route == 'terminal':
                command.append('--both')
            # run --target is compile-only even when it names the host.
            command.append(str(ROOT / 'main.omg'))
            if native_executable:
                assert route == 'native', 'executable reuse is native-only'
                command = [native_executable]
            result = subprocess.run(command, input=(ROOT / 'data' / (name + '.stdin')).read_bytes(),
                                    capture_output=True, timeout=300)
            output = destination / (name + '.stdout')
            output.write_bytes(result.stdout)
            (destination / (name + '.stderr')).write_bytes(result.stderr)
            record = dict(case=name, command=command, exit=result.returncode,
                          stdout_sha256=hashlib.sha256(result.stdout).hexdigest())
            records.append(record)
            (destination / 'records.json').write_text(json.dumps(records, indent=2) + '\n')
            with localcontext() as context:
                context.prec = 70
                check(name, output, result.returncode)
            if route == 'terminal':
                stderr = result.stderr.decode(errors='replace')
                assert f'interp exit: {case_exit(oracle, name)}' in stderr, stderr
                assert 'DECLINED' not in stderr and 'DIVERGENCE' not in stderr, stderr
            print('verified ' + route + ' ' + name, flush=True)
    (destination / 'records.json').write_text(json.dumps(records, indent=2) + '\n')


def case_exit(oracle, name):
    return oracle['cases'][name]['exit']


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', required=True)
    parser.add_argument('--target', default='linux_x86_64')
    parser.add_argument('--route', choices=['check', 'native', 'terminal', 'invalid'], required=True)
    parser.add_argument('--case', action='append')
    parser.add_argument('--native-executable', help='reuse the verified retained native application')
    arguments = parser.parse_args()
    try:
        run(arguments.omega, arguments.target, arguments.route, arguments.case,
            arguments.native_executable)
    except (AssertionError, subprocess.TimeoutExpired) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
