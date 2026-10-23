#!/usr/bin/env python3
"""Build and exercise the Linux fixture; this supplies no compiler semantics."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

LIBRARY = 'libomega_retention_fixture.so'
CONTROLS = ['exhausted', 'null_root', 'stale', 'duplicate_register',
            'duplicate_complete', 'release_live', 'null_output',
            'duplicate_release', 'stale_pending']

def invalid_control(library, case):
    import resource
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    provider = ctypes.CDLL(str(library))
    provider.retention_acquire.restype = ctypes.c_uint64
    provider.retention_register.restype = ctypes.c_uint64
    provider.retention_register.argtypes = [ctypes.c_uint64, ctypes.POINTER(ctypes.c_uint64)]
    provider.retention_complete.restype = ctypes.c_uint64
    provider.retention_complete.argtypes = provider.retention_register.argtypes
    provider.retention_release.argtypes = [ctypes.c_uint64]
    capacity = provider.retention_acquire()
    value, observed = ctypes.c_uint64(37), ctypes.c_uint64()
    pointer = ctypes.byref(value)
    output = ctypes.byref(observed)
    if case == 'exhausted':
        provider.retention_acquire()
        provider.retention_acquire()
    elif case == 'null_root':
        provider.retention_register(capacity, None)
    elif case == 'stale':
        provider.retention_release(capacity)
        provider.retention_acquire()
        provider.retention_register(capacity, pointer)
    elif case == 'duplicate_register':
        provider.retention_register(capacity, pointer)
        provider.retention_register(capacity, pointer)
    elif case == 'duplicate_complete':
        pending = provider.retention_register(capacity, pointer)
        provider.retention_complete(pending, output)
        provider.retention_complete(pending, output)
    elif case == 'release_live':
        provider.retention_register(capacity, pointer)
        provider.retention_release(capacity)
    elif case == 'null_output':
        pending = provider.retention_register(capacity, pointer)
        provider.retention_complete(pending, None)
    elif case == 'duplicate_release':
        provider.retention_release(capacity)
        provider.retention_release(capacity)
    elif case == 'stale_pending':
        old = provider.retention_register(capacity, pointer)
        provider.retention_complete(old, output)
        provider.retention_register(capacity, pointer)
        provider.retention_complete(old, output)
    raise SystemExit('invalid lifecycle transition returned normally')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--cc', default=os.environ.get('CC', 'cc'))
    parser.add_argument('--control', choices=CONTROLS)
    parser.add_argument('--library', type=Path)
    arguments = parser.parse_args()
    if platform.system() != 'Linux':
        parser.error('this fixture currently supplies a Linux ELF provider only')
    if arguments.control:
        if arguments.library is None:
            parser.error('--control requires --library')
        invalid_control(arguments.library.resolve(), arguments.control)
    if arguments.library is not None:
        parser.error('--library is only used with --control')
    fixture = Path(__file__).resolve().parent
    output = (arguments.output or fixture.parents[4] / 'build/foreign_retained_fixture').resolve()
    output.mkdir(parents=True, exist_ok=True)
    executable, library = output / 'provider-test', output / LIBRARY
    common = [arguments.cc, '-std=c11', '-Wall', '-Wextra', '-Werror', '-O2']
    commands = [common + ['-DRETENTION_FIXTURE_TEST', str(fixture / 'provider.c'), '-o', str(executable)],
                common + ['-fPIC', '-shared', str(fixture / 'provider.c'),
                          '-Wl,--version-script=' + str(fixture / 'provider.map'), '-o', str(library)],
                [str(executable)]]
    for command in commands:
        result = subprocess.run(command, capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise SystemExit(result.stdout + result.stderr)
    controls = []
    for case in CONTROLS:
        command = [sys.executable, str(Path(__file__).resolve()), '--control', case, '--library', str(library)]
        result = subprocess.run(command, capture_output=True, text=True, timeout=10)
        if result.returncode != -4:
            raise SystemExit(f'{case}: expected SIGILL, observed {result.returncode}: {result.stderr}')
        controls.append({'case': case, 'exit': result.returncode})
    print(json.dumps({'host': platform.platform(), 'library': str(library),
                      'library_sha256': hashlib.sha256(library.read_bytes()).hexdigest(),
                      'provider_sha256': hashlib.sha256((fixture / 'provider.c').read_bytes()).hexdigest(),
                      'selftest_exit': 0, 'commands': commands, 'controls': controls}, indent=2))

if __name__ == '__main__':
    main()
