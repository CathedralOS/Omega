#!/usr/bin/env python3
"""Compare two input limits on a frozen whole-product request (Linux x86-64)."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import struct
import subprocess
import sys
import time


MIB = 1024 * 1024


def replace_member(source, member, replacement):
    if source.count(member) != 1:
        raise ValueError('expected one occurrence of the bound source member')
    return source.replace(member, replacement)


def member_sum(data):
    value = 0
    for byte in data:
        value = (257 * value + byte) % (1 << 64)
    return str(value).encode('ascii')


def compile_with(compiler, source, output):
    with output.open('xb') as stream:
        subprocess.run([compiler], input=source, stdout=stream, check=True)
    output.chmod(0o700)


def measure(executable, request, output):
    # The executable reserves 128 GiB of virtual addresses. Apply the host
    # guard to resident pages. wait4 records the final peak for this child.
    start = time.monotonic()
    guard = None
    cgroup = Path('/sys/fs/cgroup')
    ceiling = (cgroup / 'memory.max').read_text().strip() if (cgroup / 'memory.max').exists() else 'max'
    error = output.with_suffix('.err')
    with request.open('rb') as stdin, output.open('xb') as stdout, error.open('xb') as stderr:
        with subprocess.Popen([executable], stdin=stdin, stdout=stdout, stderr=stderr) as child:
            status_path = Path(f'/proc/{child.pid}/status')
            while True:
                pid, status, usage = os.wait4(child.pid, os.WNOHANG)
                if pid:
                    break
                rss = next((int(line.split()[1]) for line in status_path.read_text().splitlines()
                            if line.startswith('VmRSS:')), 0)
                if rss >= 8 * MIB:
                    guard = '8 GiB RSS'
                elif ceiling != 'max' and int((cgroup / 'memory.current').read_text()) > int(ceiling) - 4 * 1024**3:
                    guard = '4 GiB cgroup reserve'
                elif time.monotonic() - start > 7200:
                    guard = 'two hours'
                if guard:
                    child.kill()
                    _, status, usage = os.wait4(child.pid, 0)
                    break
                time.sleep(0.12)
            child.returncode = os.waitstatus_to_exitcode(status)
    print(json.dumps(dict(executable=executable.name, raw_status=child.returncode, guard=guard,
                          seconds=round(time.monotonic() - start, 3), peak_kib=usage.ru_maxrss,
                          stdout_bytes=output.stat().st_size, stderr_bytes=error.stat().st_size)), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('frozen', type=Path, help='product checkout with source-constructed native Gamma and Delta')
    parser.add_argument('evaluator', type=Path, help='checkout supplying the Epsilon evaluator and its entry')
    parser.add_argument('output', type=Path, help='new directory for the request and constructed executables')
    parser.add_argument('--measure', action='store_true', help='also run the whole request under an 8 GiB RSS guard')
    args = parser.parse_args()
    frozen, evaluator, work = (path.resolve() for path in (args.frozen, args.evaluator, args.output))
    work.mkdir()
    sys.path.insert(0, str(frozen / 'bootstrap/compiled_vm'))
    vm = runpy.run_path(str(frozen / 'bootstrap/compiled_vm/test.py'))
    constructors = frozen / 'build/independent-successors'
    report = json.loads((constructors / 'report.json').read_bytes())
    gamma = frozen / 'build/independent-native-reference/compiler2'
    delta = constructors / 'delta'
    assert report['state'] == 'passed'
    for path, key in [(gamma, 'native_compiler_elf'), (delta, 'delta_elf'), (constructors / 'delta.gamma', 'delta_source')]:
        assert hashlib.sha256(path.read_bytes()).hexdigest() == report[key]['sha256'], path

    # The product and bootstrap compiler both come from the frozen checkout.
    product, manifest = runpy.run_path(str(frozen / 'source/omega/construct.py'))['compiler_input']()
    bootstrap = 'bootstrap/interpreted'
    program = vm['pack'](f'{bootstrap}/5_omega/omega_compiler.epsilon.sources', work / 'bootstrap.epsilon')
    program += (frozen / bootstrap / 'construct_omega.epsilon').read_bytes()
    closure = vm['pack'](evaluator / bootstrap / '4_epsilon/epsilon_compiler.delta.sources', work / 'epsilon.delta')
    entry = closure + (evaluator / 'tests/epsilon/evaluator-entry/evaluator_entry.delta').read_bytes()
    request = vm['epsilon_request'](closure, program, product)
    (work / 'whole.ereq').write_bytes(request)
    print(f"{len(manifest['units'])} product units, {len(request)} envelope bytes, sha256 {hashlib.sha256(request).hexdigest()}")

    # Delta authenticates each support member using its own source table.
    # Only the equal-width input bound and the adapter's corresponding sum change.
    delta_root = frozen / bootstrap / '3_delta'
    adapter = (delta_root / 'support/adapter.gamma').read_bytes()
    extended = replace_member(adapter, b'(lt 4194304 (input))', b'(lt 8388608 (input))')
    custody = (delta_root / 'implementation/emission/support.gamma').read_bytes()
    updated = replace_member(custody, member_sum(adapter), member_sum(extended))
    compile_with(gamma, replace_member((constructors / 'delta.gamma').read_bytes(), custody, updated), work / 'delta-8')
    support = (constructors / 'support.bin').read_bytes()
    dcreq = b'DCREQ\x01\0\0' + struct.pack('<II', 1, len(entry)) + entry
    canonical = subprocess.run([delta], input=dcreq + support, stdout=subprocess.PIPE, check=True).stdout
    canonical_path = work / 'published.gamma'
    canonical_path.write_bytes(canonical)
    subprocess.run(['sh', '-eu', '-c', '. ./tools/bootstrap/paths.sh\n. ./tools/bootstrap/epsilon/evaluator_env.sh\n'
                    'require_epsilon_evaluator_entry_receipt_identity "$1"', 'evaluator', canonical_path],
                   cwd=evaluator, env={**os.environ, 'OMEGA_REPO_ROOT': str(evaluator)}, check=True)
    widened = subprocess.run([work / 'delta-8'], input=dcreq + replace_member(support, adapter, extended),
                             stdout=subprocess.PIPE, check=True).stdout
    assert widened == replace_member(canonical, adapter, extended), 'unexpected change to generated Gamma'
    (work / 'extended.gamma').write_bytes(widened)
    for limit, source in [(4, canonical), (8, widened)]:
        executable = work / f'epsilon-{limit}'
        compile_with(gamma, source, executable)
        test = vm['epsilon_request'](closure, b'', b'')
        fixture = frozen / 'tests/epsilon/interpreted-omega-experiment/console_read_bytes.epsilon'
        echo = vm['epsilon_request'](closure, fixture.read_bytes(), bytes.fromhex('0080ff'))
        assert vm['execute'](executable, echo) == (0, bytes.fromhex('00000000000080ff41'))
        assert vm['execute'](executable, test) == (0, bytes.fromhex('020800000000'))
        trailing = struct.pack('<8sBBHIQQQ', b'\xffEEOUT\x01\0', 1, 1, 0, 7, 52, 0, 0)
        assert vm['execute'](executable, test.ljust(limit * MIB, b'\0')) == (0, trailing)
        assert vm['execute'](executable, test.ljust(limit * MIB + 1, b'\0')) == (253, b'')
        if args.measure:
            measure(executable, work / 'whole.ereq', work / f'whole-{limit}.out')


if __name__ == '__main__':
    main()
