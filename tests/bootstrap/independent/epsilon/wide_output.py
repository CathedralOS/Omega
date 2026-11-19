#!/usr/bin/env python3
"""Construct private Delta output and source-admission profiles for the whole compiled Epsilon customer."""
import argparse
import hashlib
import json
from pathlib import Path
import resource
import runpy
import struct
import subprocess


MIB = 1024**2
STOCK = 4 * MIB
WIDE_OUTPUT = 128 * MIB
WIDE_SOURCE = 64 * MIB


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


def identity(data):
    return {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def header(length):
    return b'DCREQ\x01\0\0' + struct.pack('<II', 1, length)


def stock_support(repo):
    support = (repo / 'build/independent-successors/support.bin').read_bytes()
    record = (repo / 'bootstrap/interpreted/3_delta/delta_compiler.composed').read_text()
    fields = dict(row.split(' ', 1) for row in record.splitlines()[1:])
    assert identity(support) == {'bytes': int(fields['support-length']), 'sha256': fields['support-sha256']}
    return support


def compile_delta(compiler, source, support):
    request = header(len(source)) + source + support
    result = subprocess.run([compiler], input=request, capture_output=True, check=True)
    assert not result.stderr and result.stdout.startswith(b'(def $application () Int 1)')
    return result.stdout


def construct(repo, out, traverse):
    stages = repo / 'build/independent-successors'
    gamma = repo / 'build/independent-native-reference/compiler2'
    delta = stages / 'delta'
    report = json.loads((stages / 'report.json').read_text())
    source = (stages / 'compiled-epsilon/compiler.delta').read_bytes()
    packed = (stages / 'delta.gamma').read_bytes()
    support = stock_support(repo)
    assert report['state'] == 'passed'
    assert identity((stages / 'compiled-epsilon/compiler').read_bytes()) == report['compiled_epsilon']['compiler_image']
    constructors = [(source, report['compiled_epsilon']['compiler_source']),
                    (packed, report['delta_source']), (delta.read_bytes(), report['delta_elf']),
                    (gamma.read_bytes(), report['native_compiler_elf'])]
    for data, expected in constructors:
        assert identity(data) == expected
    root = repo / 'bootstrap/interpreted/3_delta'
    adapter = (root / 'support/adapter.gamma').read_bytes()
    expanded = replace_member(adapter, b'(lt 4194304 n)', b'(lt 134217728 n)')
    if traverse:
        writer = Path(__file__).with_name('bytes_publisher.gamma').read_bytes()
        expanded = writer + b'\n' + replace_member(expanded, b'($pbw v 0 n)', b'($byte_walk v 0 0)')
    wide_support = replace_member(support, adapter, expanded)
    custody = (root / 'implementation/emission/support.gamma').read_bytes()
    # The adapter is the last bound member, so member offsets stay the same.
    custody_wide = replace_member(custody, member_sum(adapter), member_sum(expanded))
    member_extent = '      {})))'
    custody_wide = replace_member(custody_wide, member_extent.format(len(adapter)).encode(),
                                  member_extent.format(len(expanded)).encode())
    extent = '(def support_packed_extent () Int {})'
    custody_wide = replace_member(custody_wide, extent.format(len(support)).encode(),
                                  extent.format(len(wide_support)).encode())
    producer = replace_member(packed, custody, custody_wide)
    (out / 'output-producer.gamma').write_bytes(producer)
    (out / 'output-support.bin').write_bytes(wide_support)
    compile_with(gamma, producer, out / 'output-producer')
    canonical = compile_delta(delta, source, support)
    candidate = compile_delta(out / 'output-producer', source, wide_support)
    assert candidate == replace_member(canonical, adapter, expanded)
    (out / 'compiler.gamma').write_bytes(candidate)
    compile_with(gamma, candidate, out / 'compiler')

    # This second Delta compiler uses the original bound support and publisher.
    # Only the declared source extent and its matching refusal's limit change.
    request = (root / 'implementation/boundary/request.gamma').read_bytes()
    wider = replace_member(request, b'(lt 4194304 length)', b'(lt 67108864 length)')
    wider = replace_member(wider, b'(compiler_failure 2 4 1 12 4194304 requested)',
                           b'(compiler_failure 2 4 1 12 67108864 requested)')
    reader = replace_member(packed, request, wider)
    (out / 'reader.gamma').write_bytes(reader)
    compile_with(gamma, reader, out / 'reader')
    for name in ['output-producer.gamma', 'output-support.bin', 'compiler.gamma',
                 'compiler', 'reader.gamma', 'reader']:
        print(name, json.dumps(identity((out / name).read_bytes())))


def calibrate(repo, out):
    gamma = repo / 'build/independent-native-reference/compiler2'
    stages = repo / 'build/independent-successors'
    stock = stages / 'delta'
    support = stock_support(repo)
    wide_support = (out / 'output-support.bin').read_bytes()
    support_dir = repo / 'bootstrap/interpreted/3_delta/support'
    preceding = (support_dir / 'bytes.gamma').read_bytes() + (support_dir / 'conformance.gamma').read_bytes()
    identity_program = b'(def main ((input Bytes)) Bytes input)\n'
    canonical = compile_delta(stock, identity_program, support)
    assert compile_delta(out / 'reader', identity_program, support) == canonical
    for compiler, limit in [(stock, STOCK), (out / 'reader', WIDE_SOURCE)]:
        for size in (limit - 1, limit, limit + 1):
            result = subprocess.run([compiler], input=header(size), capture_output=True)
            refusal = (2, 4, 0, 1, 12, limit, size) if size > limit else (1, 4, 0, 1, 16, 0, 0)
            assert not result.stderr and result.returncode == refusal[0]
            assert result.stdout == struct.pack('<8sBBHIQQQ', b'\xffDCOUT\x01\0', *refusal)
    large_source = identity_program.ljust(STOCK + 1, b' ')
    rejected = subprocess.run([stock], input=header(len(large_source)) + large_source + support, capture_output=True)
    frame = struct.pack('<8sBBHIQQQ', b'\xffDCOUT\x01\0', 2, 4, 0, 1, 12, STOCK, len(large_source))
    assert rejected.returncode == 2 and not rejected.stderr and rejected.stdout == frame
    assert compile_delta(out / 'reader', large_source, support) == canonical
    append = b'(def main ((input Bytes)) Bytes (bytes_concat input (bytes_single 111)))\n'
    repeated = b'''(def repeat_bytes ((value Bytes) (count Int)) Bytes
  (if (eq count 0) value (repeat_bytes (bytes_concat value value) (- count 1))))
(def padding ((count Int)) Bytes
  (if (eq count 0) (bytes_empty) (bytes_concat (bytes_single 63) (padding (- count 1)))))
(def main ((input Bytes)) Bytes
  (bytes_concat (repeat_bytes input 5) (padding (bytes_get input 0))))
'''
    scratch = out / 'controls'
    scratch.mkdir()
    profiles = [('stock', stock, support, STOCK), ('wide', out / 'output-producer', wide_support, WIDE_OUTPUT)]
    for label, producer, bound, limit in profiles:
        assert bound.startswith(preceding)
        programs = [('identity', identity_program), ('append', append)]
        if label == 'wide':
            programs.append(('repeat', repeated))
        for name, program in programs:
            executable = scratch / f'{label}-{name}'
            compile_with(gamma, compile_delta(producer, program, bound), executable)
            for step in (-1, 0, 1):
                if name == 'repeat':
                    size, copies, suffix = STOCK - 1, 32, b'?' * (32 + step)
                else:
                    size, copies, suffix = STOCK + step, 1, b''
                    if name == 'append':
                        size -= 1
                        suffix = b'o'
                data = bytearray((bytes(range(256)) * ((size + 255) // 256))[:size])
                if name == 'repeat':
                    data[0] = len(suffix)
                expected_size = size * copies + len(suffix)
                status = 253 if size > STOCK else (254 if expected_size > limit else 0)
                check_output(executable, scratch / 'bytes', data, copies, suffix, status)
        corrupt = bound[:-1] + bytes([bound[-1] ^ 1])
        result = subprocess.run([producer], input=header(len(append)) + append + corrupt, capture_output=True)
        coordinate = 16 + len(append) + len(preceding)
        expected = struct.pack('<8sBBHIQQQ', b'\xffDCOUT\x01\0', 1, 5, 0, 2, coordinate, 0, 0)
        assert result.returncode == 1 and not result.stderr and result.stdout == expected
    check_publisher(repo, out, scratch)


def check_publisher(repo, out, scratch):
    stages = repo / 'build/independent-successors/compiled-epsilon'
    for name in ['invalid', 'lexer-digits', 'lexical', 'unsupported-nominal']:
        source = (stages / f'{name}.epsilon').read_bytes()
        expected = (stages / f'{name}.observation').read_bytes()
        observed = subprocess.run([out / 'compiler'], input=source, capture_output=True, check=True)
        assert not observed.stderr and observed.stdout == expected, name
        print(name, json.dumps(identity(observed.stdout)))

    tree = b'''(def chain ((value Bytes) (count Int) (left Int) (sofar Bytes)) Bytes
  (if (eq count 0) sofar
    (chain value (- count 1) left (if left (bytes_concat sofar value) (bytes_concat value sofar)))))
(def main ((input Bytes)) Bytes
  (if (eq (bytes_length input) 0) (bytes_empty)
    (chain (bytes_concat input (bytes_single 111)) 1024 (bytes_get input 0) (bytes_empty))))
'''
    support = (out / 'output-support.bin').read_bytes()
    gamma = compile_delta(out / 'output-producer', tree, support)
    program = scratch / 'tree'
    compile_with(repo / 'build/independent-native-reference/compiler2', gamma, program)
    for name, data in [('empty', b''), ('left', bytes(range(1, 33))), ('right', bytes(range(32)))]:
        observed = subprocess.run([program], input=data, capture_output=True, check=True)
        expected = (data + b'o') * 1024 if data else b''
        assert not observed.stderr and observed.stdout == expected, name
        print(name, json.dumps(identity(observed.stdout)))


def check_output(executable, output, data, copies, suffix, status):
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    with output.open('wb') as stream:
        result = subprocess.run([executable], input=data, stdout=stream, stderr=subprocess.PIPE)
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    assert result.returncode == status and not result.stderr
    expected = hashlib.sha256()
    if status == 0:
        for _ in range(copies):
            expected.update(data)
        expected.update(suffix)
    with output.open('rb') as stream:
        actual = hashlib.file_digest(stream, 'sha256').hexdigest()
    assert output.stat().st_size == (len(data) * copies + len(suffix) if status == 0 else 0)
    assert actual == expected.hexdigest()
    cpu = after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime
    print(executable.name, len(data), result.returncode, output.stat().st_size, actual, f'{cpu:.3f}s CPU')
    output.unlink()


def frame(repo, observation, out):
    composition = runpy.run_path(str(repo / 'tests/bootstrap/independent/epsilon/composition.py'))
    module, runtime = composition['decode_module'](observation.read_bytes())
    driver = b'(def main ((input Bytes)) Bytes (epsilon_compiled_entry input))\n'
    target = composition['program_source'](module, driver, runtime)
    (out / 'module.delta').write_bytes(module)
    (out / 'program.delta').write_bytes(target)
    support = stock_support(repo)
    (out / 'program.dcreq').write_bytes(header(len(target)) + target + support)
    for name in ['module.delta', 'program.delta', 'program.dcreq']:
        print(name, json.dumps(identity((out / name).read_bytes())))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('repo', type=Path, help='source snapshot with constructed independent successors')
    parser.add_argument('output', type=Path, help='new output directory')
    parser.add_argument('--traverse', action='store_true', help='walk the returned byte tree in output order')
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--calibrate', action='store_true', help='also run input and output boundary controls')
    modes.add_argument('--frame', type=Path, metavar='OBSERVATION',
                       help='frame an emitted module for the Delta reader')
    args = parser.parse_args()
    if args.frame and args.traverse:
        parser.error('--traverse selects the publisher during construction')
    repo, out = args.repo.resolve(), args.output.resolve()
    out.mkdir()
    if args.frame:
        frame(repo, args.frame, out)
    else:
        construct(repo, out, args.traverse)
        if args.calibrate:
            calibrate(repo, out)


if __name__ == '__main__':
    main()
