#!/usr/bin/env python3
"""Build and exercise the matching-host C fixture; supplies no compiler semantics."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess

LIBRARIES = {'Linux': 'libomega_retention_fixture.so',
             'Darwin': 'libomega_retention_fixture.dylib',
             'Windows': 'omega_retention_fixture.dll'}
CONTROLS = ['exhausted', 'null_root', 'stale', 'duplicate_register',
            'duplicate_complete', 'release_live', 'null_output',
            'duplicate_release', 'stale_pending']

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
    output = (arguments.output or fixture.parents[4] / 'build/foreign_retained_fixture').resolve()
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
    commands = [common + ['-DRETENTION_FIXTURE_TEST', str(fixture / 'provider.c'), '-o', str(executable)],
                common + shared + [str(fixture / 'provider.c'), '-o', str(library)],
                [str(executable)]]
    for command in commands:
        result = subprocess.run(command, capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise SystemExit(result.stdout + result.stderr)
    controls = []
    # Clang/GCC lower the deliberate trap to UD2 on x86-64 and BRK on ARM64.
    # An ordinary failure exit or access violation is not this lifecycle oracle.
    trap = -signal.SIGTRAP if machine in ('arm64', 'aarch64') else -signal.SIGILL
    for case in CONTROLS:
        # A native executable observes the actual deliberate trap. Windows
        # ctypes catches SEH as a Python OSError instead of terminating with
        # the native exception status, so it is not this process-exit oracle.
        command = [str(executable), case]
        result = subprocess.run(command, capture_output=True, text=True, timeout=10)
        expected_trap = ((result.returncode & 0xffffffff) == 0xc000001d
                         if system == 'Windows' else result.returncode == trap)
        if not expected_trap:
            raise SystemExit(f'{case}: expected the native deliberate trap, '
                             f'observed {result.returncode}: {result.stderr}')
        controls.append({'case': case, 'exit': result.returncode})
    print(json.dumps({'host': platform.platform(), 'library': str(library),
                      'library_sha256': hashlib.sha256(library.read_bytes()).hexdigest(),
                      'provider_sha256': hashlib.sha256((fixture / 'provider.c').read_bytes()).hexdigest(),
                      'selftest_exit': 0, 'commands': commands, 'controls': controls}, indent=2))

if __name__ == '__main__':
    main()
