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

ROOT = Path(__file__).resolve().parents[3]
COMPILER = ROOT / 'bootstrap/independent/epsilon'
SPEC = importlib.util.spec_from_file_location('epsilon_composition', COMPILER / 'composition.py')
composition = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(composition)
FIXTURES = Path(__file__).resolve().parent
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


def digit_scanner(base, boundary, program, observe, reference, compiler):
    lexer = (base / 'lexer.epsilon').read_bytes()

    def methods(first, after):
        return lexer[lexer.index(b'machine OmegaLexScan::' + first + b'('):
                     lexer.index(b'machine OmegaLexScan::' + after + b'(')]

    source = ((base / 'representations.epsilon').read_bytes()
              + (base / 'lexical_classification.epsilon').read_bytes()
              + lexer[:lexer.index(b'machine OmegaLexScan::reject(')]
              + methods(b'emit', b'span_equals') + methods(b'consume_digits', b'set_punctuation')
              + boundary + (FIXTURES / 'lexer_digits_main.epsilon').read_bytes())

    def owner_span(name):
        start = source.index(b'data ' + name + b' {') + len(b'data ')
        return start, start + len(name)

    def case_span(owner, name):
        low, _ = owner_span(owner)
        member = b'case ' + name + b';'
        start = source.index(member, low, source.index(b'}', low))
        return start, start + len(member)

    main_low, main_high = owner_span(b'Main')
    main_type = f'(EpsilonNamedType 0 0 (EpsilonNameSpan {main_low} {main_high}))'
    entry = ((FIXTURES / 'lexer_digits_driver.delta').read_bytes()
             + f'(def main ((input Bytes)) Bytes (test_digits_entry {main_type} input))\n'.encode())
    target, report = program(source, 'lexer-digits', entry)
    expected = b'\0' + struct.pack('<i', 42)
    assert observe(target) == reference(source, b'', 'lexer-digits-source') == expected

    # The same spelling in a foreign owner fails checking at the authored call.
    call = b'omega_digit_in_base(49, copy[0])'
    offset = source.index(call)
    invalid = source.replace(call, b'omega_digit_in_base(49, self.other)', 1)
    assert observe(compiler, invalid) == b'rejected 18 ' + str(offset).encode()
    assert reference(invalid, b'', 'lexer-digits-wrong-owner') == b'\x02\x12' + struct.pack('<I', offset)

    # Bypass the Epsilon checker at the target interface to exercise states
    # impossible in valid source. A wildcard may only accept a known member.
    number = b'OmegaNumericBase'
    owner = owner_span(number)
    owner_end = source.index(b'}', owner[0]) + 1
    internal = b'\xffEEOUT\1\0' + bytes((3, 2, 0, 0)) + struct.pack('<IQQQ', 1, owner_end, 0, 0)
    binary = case_span(number, b'Binary')
    cases = [
        ('binary', 0, binary, b'\0'), ('octal', 0, case_span(number, b'Octal'), b'\1'),
        ('decimal', 0, case_span(number, b'Decimal'), b'\1'), ('default-binary', 1, owner, b'\0'),
        ('foreign-case', 0, case_span(b'OtherBase', b'Binary'), internal),
        ('foreign-default', 1, owner_span(b'OtherBase'), internal),
        ('wrong-end', 0, (binary[0], binary[1] - 1), internal),
        ('wrong-start', 0, (binary[0] + 1, binary[1]), internal),
        ('supplied-payload', 2, binary, internal), ('declared-payload', 3, binary, internal),
        ('record-children', 4, owner, internal), ('indexed-children', 5, owner, internal),
        ('scalar', 6, owner, internal),
    ]
    report['cases'] = []
    for label, kind, span, expected in cases:
        actual = observe(target, struct.pack('<BII', kind, *span))
        assert actual == expected, (label, actual, expected)
        report['cases'].append({'name': label, 'output': identity(actual)})
    return report


def stored_customers(work, observe, compile_delta, reference, compiler):
    boundary = CONSOLE[:CONSOLE.index(b'data Main')]
    driver = b'(def main ((input Bytes)) Bytes (epsilon_compiled_entry input))\n'

    def program(source, name, entry=driver):
        (work / f'{name}.epsilon').write_bytes(source)
        observation = observe(compiler, source)
        (work / f'{name}.observation').write_bytes(observation)
        generated, runtime = composition.decode_module(observation)
        assert runtime == 'storage', observation[:80]
        target = composition.program_source(generated, entry, runtime)
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

    # The production keyword classifier compares its actual literal views.
    lexer = (base / 'lexer.epsilon').read_bytes()
    source = ((base / 'representations.epsilon').read_bytes()
              + lexer[:lexer.index(b'machine OmegaLexScan::scan_whitespace(')] + boundary
              + (FIXTURES / 'lexer_diagnostic_main.epsilon').read_bytes())
    main_start = source.index(b'Main::main(')
    span = f'{main_start} {main_start + len("Main")}'
    main_type = f'(EpsilonNamedType {span} (EpsilonNameSpan {span}))'
    entry = ((FIXTURES / 'lexer_case_driver.delta').read_bytes()
             + f'(def main ((input Bytes)) Bytes (test_lexer_entry {main_type} input))\n'.encode())
    executable, diagnostic_report = program(source, 'lexer-diagnostic', entry)

    def case_span(owner, name):
        owner_start = source.index(b'data ' + owner + b' {')
        owner_end = source.index(b'}', owner_start)
        declaration = b'case ' + name + b';'
        start = source.index(declaration, owner_start, owner_end)
        return struct.pack('<II', start, start + len(declaration))

    expected_exit = b'\0' + struct.pack('<i', 42)
    diagnostic_report['cases'] = []
    keyword = case_span(b'OmegaTokenKind', b'Keyword')
    identifier = case_span(b'OmegaTokenKind', b'Identifier')
    for label, word, observed_cases in [
            ('empty', b'', b''), ('as', b'as', keyword + case_span(b'OmegaKeywordKind', b'As')),
            ('copy', b'copy', keyword + case_span(b'OmegaKeywordKind', b'Copy')),
            ('identifier', b'aside', identifier),
            ('bounds', b'\xff', identifier)]:
        expected = b'\x01\x06' if label == 'bounds' else expected_exit
        assert reference(source, word, f'lexer-diagnostic-{label}') == expected
        output = observe(executable, word)
        # The token after a refused empty span and the keyword of an identifier
        # are unspecified. Only check fields committed by the lexer.
        required = expected + keyword + observed_cases
        assert output.startswith(required) and len(output) - len(expected) in (8, 16, 24), (label, output)
        diagnostic_report['cases'].append({'input': identity(word), 'output': identity(output)})

    malformed = source.replace(br'\x00\t', br'\x0g\t', 1)
    offset = malformed.index(br'\x0g')
    assert observe(compiler, malformed) == b'rejected 5 ' + str(offset).encode()
    assert reference(malformed, b'', 'lexer-invalid-string') == b'\x02\x05' + struct.pack('<I', offset)

    # Substitute a different sum owner at the same call and check its exact diagnostic.
    offset = source.index(b'self.scanner.reject(self.diagnostic,')
    invalid = source.replace(b'reject(self.diagnostic,', b'reject(self.scanner.token_kind,', 1)
    assert observe(compiler, invalid) == b'rejected 18 ' + str(offset).encode()
    assert reference(invalid, b'', 'lexer-diagnostic-wrong-owner') == b'\x02\x12' + struct.pack('<I', offset)

    offset = source.index(b'self.scanner.set_keyword(OmegaKeywordKind::As)')
    invalid = source.replace(b'self.scanner.set_keyword(OmegaKeywordKind::As)',
                             b'self.scanner.set_keyword(OtherKeyword::As)', 1)
    assert observe(compiler, invalid) == b'rejected 18 ' + str(offset).encode()
    assert reference(invalid, b'', 'stored-case-wrong-owner') == b'\x02\x12' + struct.pack('<I', offset)

    payload = source + (b'\ndata Number { case Box(value: i32); }\n'
                        b'machine Main::save_number(&mut self) { let number: Number = Number::Box(7); }\n')
    offset = payload.index(b'Number::Box(7)')
    assert observe(compiler, payload) == b'unsupported ' + str(offset).encode()
    assert reference(payload, b'', 'stored-case-payload') == expected_exit

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

    # The live value is Marker::Clear. The original literal is Marker::Set.
    offset = source.index(b'Marker::Set) == 1')
    live = source[:offset] + source[offset:].replace(b'Marker::Set', b'tag', 1)
    executable, _ = program(live, 'stored-free-argument')
    assert observe(executable, b'\2') == reference(live, b'\2', 'stored-free-reference') == b'\1\10'
    digits = digit_scanner(base, boundary, program, observe, reference, compiler)
    # Echo all decoded octets from one source literal, crossing each control,
    # delimiter and non-ASCII byte as data in the generated Delta program.
    octets = bytes(range(256))
    encoded = b''.join(f'\\x{byte:02x}'.encode() for byte in octets)
    fixture = (FIXTURES / 'literal_bytes_main.epsilon').read_bytes()
    assert fixture.count(b'"test bytes"') == 1
    source = CONSOLE + fixture.replace(b'"test bytes"', b'"' + encoded + b'"')
    executable, literal_report = program(source, 'literal-bytes')
    expected = expected_exit + octets
    assert observe(executable) == reference(source, b'', 'literal-bytes-reference') == expected
    literal_report['output'] = identity(expected)
    return {'request': request_report, 'lexer_diagnostic': diagnostic_report,
            'storage': storage_report, 'literal_bytes': literal_report, 'lexer_digits': digits}


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
        enum_spec = importlib.util.spec_from_file_location('epsilon_enum_controls', COMPILER / 'enum_test.py')
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
