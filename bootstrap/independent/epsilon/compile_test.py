"""Construct and exercise actual Epsilon source compiled through Delta/native.

Customers include the unchanged lexical and request/UTF8 production modules.
Receiver storage and views use direct functions over shared runtime leaves.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import struct
import time
import traceback

SPEC = importlib.util.spec_from_file_location('epsilon_composition', Path(__file__).with_name('composition.py'))
composition = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(composition)
ROOT = composition.ROOT
FIXTURES = ROOT / 'tests/epsilon/compiled-native'
CONSOLE = b'''boundary trait Console {
 machine exit_process(return_code: i32) -> never;
 machine write_byte(value: i32);
 machine read_byte() -> i32;
 machine write_line(text: &[u8]);
}
data Main { console: Console; }
'''
EMPTY_MAIN = b'machine Main::main(&mut self) {}\n'
CONTROLS = [
    ('count', '5', 5), ('call', '5', 6),
    ('add_overflow', '', ('trap', 1)),
    ('subtract_overflow', '', ('trap', 1)),
    ('multiply_overflow', '', ('trap', 1)),
    ('negate_overflow', '', ('trap', 1)),
    ('divide_zero', '', ('trap', 2)),
    ('remainder_zero', '', ('trap', 2)),
    ('divide_overflow', '', ('trap', 3)),
    ('remainder_overflow', '', ('trap', 3)),
    ('shift_range', '', ('trap', 4)),
    ('shift_negative', '', ('trap', 4)),
    ('short_and', '', 0), ('short_or', '', 1),
    ('bad_guard', '', ('trap', 7)), ('bad_right_guard', '', ('trap', 7)),
    ('no_arm', '', ('trap', 9)), ('bitwise', '', -2147483393),
    ('bit_or', '', 19), ('compare', '', 4),
]


def identity(data):
    return {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def stored_customers(work, observe, compile_delta, reference, compiler):
    boundary = CONSOLE[:CONSOLE.index(b'data Main')]
    driver = b'(def main ((input Bytes)) Bytes (epsilon_compiled_entry input))\n'

    def program(source, name):
        (work / f'{name}.epsilon').write_bytes(source)
        observation = observe(compiler, source)
        (work / f'{name}.observation').write_bytes(observation)
        generated, runtime = composition.decode_module(observation)
        assert runtime == 'storage', observation[:80]
        target = composition.program_source(generated, driver, runtime)
        executable = compile_delta(target, name)
        return executable, {'source': identity(source), 'generated': identity(generated),
                            'target': identity(target), 'image': identity(executable.read_bytes())}

    base = ROOT / 'bootstrap/interpreted/5_omega'
    # Complete existing production owners, with no body extraction or rewriting.
    source = ((base / 'representations.epsilon').read_bytes()
              + (base / 'request_and_utf8.epsilon').read_bytes() + boundary
              + (FIXTURES / 'request_main.epsilon').read_bytes())
    executable, request_report = program(source, 'request-target')
    spec = importlib.util.spec_from_file_location('omega_request_fixture', ROOT / 'tests/bootstrap/omega-executable/gate.py')
    fixture = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fixture)
    valid = fixture.ocreq_request(b'machine main() -> u8 { 42 }', b'program.omg')
    request_report['cases'] = []
    for name, payload in [('ocreq', valid), ('truncated', valid[:-1]), ('empty', b''),
                          ('identity', b'X' + valid[1:]),
                          ('unicode', b'\xf0\x90\x80\x80'), ('surrogate', b'\xed\xa0\x80')]:
        actual = observe(executable, payload)
        expected = reference(source, payload, f'request-reference-{name}')
        assert actual == expected and actual[:5] == b'\0' + struct.pack('<i', 42), (name, actual, expected)
        fields = struct.unpack('<14i', actual[5:])
        if name == 'ocreq':
            assert fields[0] == 1 and fields[6] == 1, fields
        if name == 'unicode':
            assert fields[12] == 1, fields
        if name == 'surrogate':
            assert fields[12] == 0 and fields[13] == 0, fields
        request_report['cases'].append({'name': name, 'input': identity(payload), 'output': identity(actual)})

    source = boundary + (FIXTURES / 'storage_main.epsilon').read_bytes()
    executable, storage_report = program(source, 'storage-target')
    storage_report['cases'] = []
    for name, payload, expected in [('live', b'\2', b'\0' + struct.pack('<i', 42) + b'A'),
                                    ('bounds', b'\10', b'\1\6A'), ('eof', b'', b'\1\6A')]:
        actual = observe(executable, payload)
        assert actual == expected == reference(source, payload, f'storage-reference-{name}'), (name, actual, expected)
        storage_report['cases'].append({'name': name, 'input': identity(payload), 'output': identity(actual)})
    # The same compiled entry takes 300,000 state transfers while retaining a
    # local array through its view. The short path above has the independent
    # source-built evaluator oracle; this longer path falsifies growing frames.
    actual = observe(executable, b'\2\1')
    assert actual == b'\0' + struct.pack('<i', 42) + b'A', actual
    storage_report['native_state_transfers'] = 300000
    return {'request': request_report, 'storage': storage_report}


def run(work, vm, producer, realize, seconds):
    work = Path(work) / 'compiled-epsilon'
    work.mkdir(parents=True, exist_ok=True)
    report = {'state': 'running', 'pid': os.getpid(), 'started': time.time(), 'stage': 'construction'}

    def checkpoint():
        report['children_maxrss_kib'] = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        pending = work / 'result.json.tmp'
        pending.write_text(json.dumps(report, indent=2) + '\n')
        pending.replace(work / 'result.json')

    def observe(executable, payload=b''):
        status, output = vm.execute(executable, payload, seconds)
        assert status == 0, (str(executable), status, output[:80])
        return output

    support = (work.parent / 'support.bin').read_bytes()

    def compile_delta(source, name):
        request = b'DCREQ\x01\0\0' + struct.pack('<II', 1, len(source)) + source + support
        (work / f'{name}.delta').write_bytes(source)
        (work / f'{name}.request').write_bytes(request)
        receipt = observe(work.parent / 'delta', request)
        (work / f'{name}.gamma').write_bytes(receipt)
        return vm.compile_program(work, receipt, name, compiler=producer, realize=realize)

    def compile_epsilon(source, name):
        (work / f'{name}.epsilon').write_bytes(source)
        observation = observe(work / 'compiler', source)
        (work / f'{name}.observation').write_bytes(observation)
        assert observation.startswith(b'compiled\n'), (name, observation[:80])
        return observation[len(b'compiled\n'):]

    def reference(source, payload, name):
        request = vm.epsilon_request(closure, source, payload)
        (work / f'{name}.request').write_bytes(request)
        observation = observe(work.parent / 'epsilon', request)
        (work / f'{name}.observation').write_bytes(observation)
        return observation

    checkpoint()
    try:
        closure = vm.pack('bootstrap/interpreted/4_epsilon/epsilon_compiler.delta.sources', work / 'evaluator.delta')
        compiler_source = composition.compiler_source(closure)
        compiler = compile_delta(compiler_source, 'compiler')
        report['compiler_source'] = identity(compiler_source)
        report['compiler_image'] = identity(compiler.read_bytes())
        runtime = composition.runtime_source()
        report['target_runtime'] = identity(runtime)
        report['stage'] = 'actual-lexical-customer'
        checkpoint()
        actual = (ROOT / 'bootstrap/interpreted/5_omega/lexical_classification.epsilon').read_bytes()
        # This is the complete unchanged prefix of four production machines.
        # The next machine needs sums, outside this declared scalar capability.
        prefix = actual[:actual.index(b'machine omega_digit_in_base(')]
        source = prefix + CONSOLE + EMPTY_MAIN
        generated = compile_epsilon(source, 'lexical')
        target = composition.program_source(generated, (FIXTURES / 'lexical_driver.delta').read_bytes())
        executable = compile_delta(target, 'lexical-target')
        payload = bytes(range(256))
        result = observe(executable, payload)
        (work / 'lexical.output').write_bytes(result)
        report['lexical'] = {'production_prefix': identity(prefix), 'source': identity(source),
                             'generated': identity(generated), 'target': identity(target),
                             'image': identity(executable.read_bytes()), 'output': identity(result)}
        checkpoint()
        expected = b''.join(bytes([
            int(c in (9, 10, 13, 32)),
            int(c == 95 or 65 <= c <= 90 or 97 <= c <= 122),
            int(c == 95 or 65 <= c <= 90 or 97 <= c <= 122 or 48 <= c <= 57),
            (c - 48 if 48 <= c <= 57 else c - 87 if 97 <= c <= 102 else c - 55 if 65 <= c <= 70 else -1) + 1,
        ]) for c in range(256))
        assert result == expected
        reference_source = prefix + CONSOLE + (FIXTURES / 'lexical_main.epsilon').read_bytes()
        observation = reference(reference_source, payload, 'lexical-reference')
        assert observation == b'\0' + struct.pack('<i', 42) + result, observation[:80]
        for label, edge_payload, expected_edge in (
            ('empty', b'', b''), ('byte-255', b'\xff', expected[-4:]),
        ):
            assert observe(executable, edge_payload) == expected_edge
            assert reference(reference_source, edge_payload, f'lexical-edge-{label}') == (
                b'\0' + struct.pack('<i', 42) + expected_edge)
        report['eof_controls'] = 2
        report['stage'] = 'scalar-controls'
        checkpoint()
        scalar_source = (FIXTURES / 'scalars.epsilon').read_bytes()
        generated = compile_epsilon(scalar_source + CONSOLE + EMPTY_MAIN, 'scalars')
        calls = [f'(ep_test_wire (ep_free_ep_test_{name} {argument}))' for name, argument, _ in CONTROLS]
        body = '""'
        for call in reversed(calls):
            body = f'(bytes_concat {call} {body})'
        driver = (FIXTURES / 'scalar_driver.delta').read_bytes()
        driver += f'\n(def main ((source Bytes)) Bytes {body})\n'.encode()
        executable = compile_delta(composition.program_source(generated, driver), 'scalar-target')
        result = observe(executable)
        expected = b''.join(b'\2' + bytes([value[1]]) if isinstance(value, tuple)
                            else b'\0' + struct.pack('<i', value)
                            for _, _, value in CONTROLS)
        assert result == expected, (result, expected)
        scalar_observations = []
        for name, argument, value in CONTROLS:
            call = f'ep_test_{name}({argument})'.encode()
            main = b'machine Main::main(&mut self) { let value: i32 = ' + call + b''';
 self.console.write_byte(value & 255);
 self.console.write_byte((value >> 8) & 255);
 self.console.write_byte((value >> 16) & 255);
 self.console.write_byte((value >> 24) & 255);
 self.console.exit_process(42); }'''
            observed = reference(scalar_source + CONSOLE + main, b'', f'reference-{name}')
            expected = (b'\1' + bytes([value[1]]) if isinstance(value, tuple)
                        else b'\0' + struct.pack('<i', 42) + struct.pack('<i', value))
            assert observed == expected, (name, observed, expected)
            scalar_observations.append({'name': name, 'observation': identity(observed)})
        report['scalar_controls'] = len(CONTROLS)
        report['scalar_observations'] = scalar_observations
        report['stage'] = 'refusals'
        checkpoint()
        invalid = scalar_source.replace(b'left - 1', b'missing - 1') + CONSOLE + EMPTY_MAIN
        refusal = observe(compiler, invalid)
        (work / 'invalid.epsilon').write_bytes(invalid)
        (work / 'invalid.observation').write_bytes(refusal)
        assert refusal == b'rejected 17 ' + str(invalid.index(b'missing')).encode(), refusal
        assert reference(invalid, b'', 'invalid-reference') == (
            b'\2\x11' + struct.pack('<I', invalid.index(b'missing')))
        # Unsupported Console text output and a payload parameter remain valid
        # Epsilon source. Neither may publish a previously emitted prefix.
        for name, suffix in (
            ('effect', b'machine Main::unused_write(&mut self) { self.console.write_line("text"); }\n'),
            ('nominal', b'data Color { case Red(value: i32); case Blue; }\nmachine nominal(value: Color) -> i32 { return 0; }\n'),
        ):
            request = source + suffix
            (work / f'unsupported-{name}.epsilon').write_bytes(request)
            refusal = observe(compiler, request)
            (work / f'unsupported-{name}.observation').write_bytes(refusal)
            assert refusal.startswith(b'unsupported ') and b'(def' not in refusal, refusal[:80]
            assert reference(request, b'', f'unsupported-{name}-reference') == (
                b'\0' + struct.pack('<i', 0))
        report['stage'] = 'complete-lexical-module'
        checkpoint()
        enum_spec = importlib.util.spec_from_file_location('epsilon_enum_controls', Path(__file__).with_name('enum_test.py'))
        enum_controls = importlib.util.module_from_spec(enum_spec)
        enum_spec.loader.exec_module(enum_controls)
        report['enum_customer'] = enum_controls.run(work, vm, producer, realize, seconds, composition, compiler)
        report['stage'] = 'actual-request-and-storage'
        checkpoint()
        report['stored_customers'] = stored_customers(work, observe, compile_delta, reference, compiler)
        report.update(state='passed', stage='complete', refusal_controls=3, finished=time.time())
        checkpoint()
        return report
    except BaseException:
        report.update(state='failed', error=traceback.format_exc(), finished=time.time())
        checkpoint()
        raise
