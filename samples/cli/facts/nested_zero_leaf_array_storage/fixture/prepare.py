#!/usr/bin/env python3
"""Build the matching-host zero-leaf record provider and run its real C self-test."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess

LIBRARIES = {
    'Linux': 'libomega_zeroleaf_fixture.so',
    'Darwin': 'libomega_zeroleaf_fixture.dylib',
    'Windows': 'omega_zeroleaf_fixture.dll',
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--cc', default=os.environ.get('CC') or
                        ('clang' if platform.system() == 'Windows' else 'cc'))
    arguments = parser.parse_args()
    system, machine = platform.system(), platform.machine().lower()
    if system not in LIBRARIES or machine not in ('x86_64', 'amd64', 'aarch64', 'arm64'):
        parser.error('the fixture supports the four hosted x86-64/ARM64 profiles')
    if system == 'Windows' and machine not in ('x86_64', 'amd64'):
        parser.error('the Windows fixture requires x86-64')
    if system == 'Darwin' and machine not in ('arm64', 'aarch64'):
        parser.error('the macOS fixture requires ARM64')
    fixture = Path(__file__).resolve().parent
    output = (arguments.output or fixture.parents[4] / 'build/foreign_zero_leaf_array_fixture').resolve()
    output.mkdir(parents=True, exist_ok=True)
    executable = output / ('provider-test.exe' if system == 'Windows' else 'provider-test')
    library = output / LIBRARIES[system]
    common = [arguments.cc, '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2']
    if system == 'Linux':
        shared = ['-fPIC', '-shared',
                  '-Wl,--version-script=' + str(fixture / 'provider.map')]
    elif system == 'Darwin':
        shared = ['-dynamiclib', '-Wl,-install_name,' + library.name]
    else:
        shared = ['-shared']
    commands = [
        common + ['-DZERO_LEAF_FIXTURE_TEST', str(fixture / 'provider.c'), '-o', str(executable)],
        common + shared + [str(fixture / 'provider.c'), '-o', str(library)],
        [str(executable)],
    ]
    for command in commands:
        result = subprocess.run(command, capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise SystemExit(result.stdout + result.stderr or
                             f'fixture command failed with exit {result.returncode}: {command}')
    print(json.dumps({
        'host': platform.platform(),
        'library': str(library),
        'library_sha256': hashlib.sha256(library.read_bytes()).hexdigest(),
        'provider_sha256': hashlib.sha256((fixture / 'provider.c').read_bytes()).hexdigest(),
        'map_sha256': hashlib.sha256((fixture / 'provider.map').read_bytes()).hexdigest(),
        'selftest_sha256': hashlib.sha256(executable.read_bytes()).hexdigest(),
        'selftest_exit': 0,
        'commands': commands,
    }, indent=2))


if __name__ == '__main__':
    main()
