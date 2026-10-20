#!/usr/bin/env python3
"""Validate integer crash-entry view semantics and their retained native images."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time

from compiler_profile import require_optimized

ROOT = Path(__file__).resolve().parents[1]
FACT = ROOT / 'samples/cli/facts/crash_entry_representation_view'
CASES = ('local', 'high_bit', 'reverse_signed', 'field', 'call_result', 'other_state',
         'snapshot_after_write', 'surviving_guard', 'view_then_widen')
CONTROLS = ('changed_guard', 'stale_storage', 'incompatible_view')


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def invoke(command):
    started = time.monotonic()
    result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=180)
    print(json.dumps({'command': list(map(str, command)), 'exit': result.returncode,
                      'seconds': time.monotonic() - started,
                      'stdout': result.stdout, 'stderr': result.stderr}), flush=True)
    return result


def runtime(binary, prepared=None, directory=None):
    manifest = {'producer_sha256': digest(binary), 'cases': {}}
    if prepared:
        assert prepared['producer_sha256'] == manifest['producer_sha256'], 'producer changed'
    for name in CASES:
        project = FACT if name == 'local' else FACT / name
        source = project / 'main.omg'
        expected = int((project / 'expected_native_exit.txt').read_text())
        assert expected == int((project / 'expected_interpreter_exit.txt').read_text()) == 0
        result = invoke([binary, 'run', '--both', '--keep', source])
        assert result.returncode == 0, f'{name}: native/Terminal execution failed'
        assert f'native exit {expected}\n' in result.stderr, f'{name}: native result missing'
        assert f'interp exit: {expected}\n' in result.stderr, f'{name}: Terminal result missing'
        kept = [Path(line.removeprefix('build dir kept: ')) for line in result.stderr.splitlines()
                if line.startswith('build dir kept: ')]
        assert len(kept) == 1, f'{name}: native product not retained'
        native = kept[0] / 'omega-program'
        assert native.is_file(), f'{name}: native image missing'
        row = {'source_sha256': digest(source), 'native_sha256': digest(native)}
        if directory:
            destination = directory / (name + '.elf')
            shutil.copy2(native, destination)
            row['artifact'] = str(destination)
        if prepared:
            prior = prepared['cases'][name]
            assert row['source_sha256'] == prior['source_sha256'], f'{name}: source changed'
            assert row['native_sha256'] == prior['native_sha256'], f'{name}: native image changed'
            assert digest(prior['artifact']) == prior['native_sha256'], f'{name}: retained image changed'
        manifest['cases'][name] = row
        print(json.dumps({'case': name, 'native_exit': expected, 'terminal_exit': expected,
                          **row}), flush=True)
    return manifest


def negatives(binary):
    for name in CONTROLS:
        project = FACT / 'controls' / name
        expected = [line for line in (project / 'expected.txt').read_text().splitlines() if line]
        assert expected, f'{name}: missing refusal oracle'
        result = invoke([binary, '--check', project / 'main.omg'])
        assert result.returncode == 1, f'{name}: invalid source was not rejected'
        assert all(fragment in result.stderr for fragment in expected), f'{name}: wrong refusal'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('prepare', 'runtime', 'negative'))
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--manifest', type=Path)
    parser.add_argument('--directory', type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    require_optimized(binary)
    if args.mode == 'prepare':
        assert args.directory and args.manifest, 'preparation needs artifact directory and manifest'
        directory = args.directory.resolve()
        directory.mkdir(parents=True, exist_ok=True)
        manifest = runtime(binary, directory=directory)
        args.manifest.write_text(json.dumps(manifest, indent=2) + '\n')
    elif args.mode == 'runtime':
        assert args.manifest, 'runtime replay needs the retained product manifest'
        runtime(binary, prepared=json.loads(args.manifest.read_text()))
    else:
        negatives(binary)


if __name__ == '__main__':
    main()
