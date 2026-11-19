"""Exercise the complete production lexical module with named nominal cases."""
import hashlib
import importlib.util
import os
import json
from pathlib import Path
import struct
import time
import traceback


def _run(work, vm, producer, realize, seconds, composition, compiler):
    root = composition.ROOT
    fixtures = root / 'tests/epsilon/compiled-native'
    work = Path(work)
    support = (work.parent / 'support.bin').read_bytes()
    closure = vm.pack('bootstrap/interpreted/4_epsilon/epsilon_compiler.delta.sources', work / 'enum-evaluator.delta')
    console = b'''boundary trait Console {
 machine exit_process(return_code: i32) -> never;
 machine write_byte(value: i32);
 machine read_byte() -> i32;
 machine write_line(text: &[u8]);
}
data Main { console: Console; }
'''
    empty_main = b'machine Main::main(&mut self) {}\n'

    def identity(data):
        return {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}

    def observe(executable, payload):
        status, result = vm.execute(executable, payload, seconds)
        assert status == 0, (str(executable), status, result[:80])
        return result

    def reference(source, payload, label):
        request = vm.epsilon_request(closure, source, payload)
        (work / f'{label}.request').write_bytes(request)
        result = observe(work.parent / 'epsilon', request)
        (work / f'{label}.observation').write_bytes(result)
        return result

    # These are exact complete production owners. No compiler implementation
    # recognizes either filename, selected byte string or digest.
    declarations = (root / 'bootstrap/interpreted/5_omega/representations.epsilon').read_bytes()
    start = declarations.index(b'data OmegaNumericBase {')
    end = declarations.index(b'data OmegaKeywordKind {', start)
    nominal = declarations[start:end]
    lexical = (root / 'bootstrap/interpreted/5_omega/lexical_classification.epsilon').read_bytes()
    forwarding = (fixtures / 'enum_forward.epsilon').read_bytes()
    # Forward type declarations in Epsilon must still precede all functions in
    # emitted Delta. Deliberately place the authored enum after its customers.
    source = lexical + forwarding + nominal + console + empty_main
    (work / 'enum.epsilon').write_bytes(source)
    started = time.monotonic()
    observation = observe(compiler, source)
    (work / 'enum.observation').write_bytes(observation)
    assert observation.startswith(b'compiled\n'), observation[:80]
    generated = observation[len(b'compiled\n'):]
    target = composition.program_source(generated, (fixtures / 'enum_driver.delta').read_bytes())
    (work / 'enum-target.delta').write_bytes(target)
    request = b'DCREQ\x01\0\0' + struct.pack('<II', 1, len(target)) + target + support
    (work / 'enum-target.request').write_bytes(request)
    receipt = observe(work.parent / 'delta', request)
    (work / 'enum-target.gamma').write_bytes(receipt)
    executable = vm.compile_program(work, receipt, 'enum-target', compiler=producer, realize=realize)
    payload = bytes(range(256))
    result = observe(executable, payload)
    (work / 'enum.output').write_bytes(result)
    expected = bytearray()
    for c in range(256):
        is_start = c == 95 or 65 <= c <= 90 or 97 <= c <= 122
        digits = [int(48 <= c <= 49), int(48 <= c <= 55), int(48 <= c <= 57),
                  int(48 <= c <= 57 or 65 <= c <= 70 or 97 <= c <= 102)]
        expected.extend([int(c in (9, 10, 13, 32)), int(is_start),
                         int(is_start or 48 <= c <= 57),
                         (c - 48 if 48 <= c <= 57 else c - 87 if 97 <= c <= 102 else c - 55 if 65 <= c <= 70 else -1) + 1,
                         *digits, *digits])
    assert result == expected, (len(result), len(expected))
    reference_source = lexical + forwarding + nominal + console + (fixtures / 'enum_main.epsilon').read_bytes()
    assert reference(reference_source, payload, 'enum-reference') == b'\0' + struct.pack('<i', 42) + result
    for label, edge in [('empty', b''), ('byte-255', b'\xff')]:
        actual = observe(executable, edge)
        assert actual == (b'' if not edge else expected[-12:])
        assert reference(reference_source, edge, f'enum-edge-{label}') == b'\0' + struct.pack('<i', 42) + actual
    # The other already constructed source-owned Gamma backend provides a
    # separate realization of this same newly emitted Gamma receipt.
    spec = importlib.util.spec_from_file_location('enum_native_construction', root / 'bootstrap/independent/source_test.py')
    construction = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(construction)
    predecessor = root / 'build/independent-native-reference'
    cold = json.loads((predecessor / 'report.json').read_text())
    alpha = predecessor / 'alpha-compiler'
    assert cold['mode'] == 'reference'
    assert cold['alpha_customer_source'] == identity(construction.source(False))
    assert cold['alpha_customer_elf'] == identity(alpha.read_bytes())

    class AlphaCompiler:
        def __init__(self, subject):
            self.subject = subject

        def compile(self):
            return observe(alpha, self.subject)

    alpha_executable = vm.compile_program(work, receipt, 'enum-alpha', compiler=AlphaCompiler, realize=vm.stamp)
    assert observe(alpha_executable, payload) == result
    # Nominal identity is checked before lowering, even when two owners use
    # identical case spellings. Equality remains scalar-only Epsilon source.
    invalid_controls = [
        ('owner', b'data Other { case Binary; case Octal; case Decimal; case Hexadecimal; }\n'
         b'machine wrong(value: Other) -> i32 { return omega_digit_in_base(0, value); }\n', b'omega_digit_in_base(0, value)'),
        ('equality', b'machine enum_equal(value: OmegaNumericBase) -> i32 { return value == value; }\n', b'value =='),
    ]
    for label, suffix, anchor in invalid_controls:
        invalid = source + suffix
        offset = len(source) + suffix.index(anchor)
        rejected = observe(compiler, invalid)
        (work / f'enum-invalid-{label}.epsilon').write_bytes(invalid)
        (work / f'enum-invalid-{label}.observation').write_bytes(rejected)
        assert rejected == b'rejected 18 ' + str(offset).encode(), (label, rejected, offset)
        assert reference(invalid, b'', f'enum-invalid-{label}-reference') == b'\2\x12' + struct.pack('<I', offset)

    unsupported_controls = [
        ('payload', b'data Carry { case Value(value: i32); case Empty; }\n'
         b'machine payload(value: Carry) -> i32 { return 0; }\n', b'Carry) ->'),
        ('return', b'machine enum_result() -> OmegaNumericBase { return OmegaNumericBase::Binary; }\n', b'OmegaNumericBase {'),
        ('wildcard', b'machine enum_default(value: OmegaNumericBase) -> i32 {\n'
         b' transition value { OmegaNumericBase::Binary -> return 1 _ -> return 0 } }\n', b'_ ->'),
    ]
    for label, suffix, anchor in unsupported_controls:
        unsupported = source + suffix
        offset = len(source) + suffix.index(anchor)
        refused = observe(compiler, unsupported)
        (work / f'enum-unsupported-{label}.epsilon').write_bytes(unsupported)
        (work / f'enum-unsupported-{label}.observation').write_bytes(refused)
        assert refused == b'unsupported ' + str(offset).encode(), (label, refused, offset)
        assert b'(def' not in refused and b'(data' not in refused
        assert reference(unsupported, b'', f'enum-unsupported-{label}-reference') == b'\0' + struct.pack('<i', 0)
    report = {'state': 'passed', 'production_lexical': identity(lexical), 'production_enum': identity(nominal),
              'source': identity(source), 'generated': identity(generated), 'target': identity(target),
              'image': identity(executable.read_bytes()), 'receipt': identity(receipt), 'output': identity(result),
              'alpha_compiler_source': cold['alpha_customer_source'], 'alpha_compiler_image': cold['alpha_customer_elf'],
              'alpha_tape': identity((work / 'enum-alpha.tape').read_bytes()),
              'byte_values': 256, 'bases': 4, 'forwarded_cases': 1024, 'eof_controls': 2, 'nominal_rejections': len(invalid_controls),
              'unsupported_controls': len(unsupported_controls),
              'elapsed_seconds': time.monotonic() - started}
    (work / 'enum-result.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def run(work, vm, producer, realize, seconds, composition, compiler):
    work = Path(work)
    report = {'state': 'running', 'pid': os.getpid(), 'started': time.time()}
    destination = work / 'enum-result.json'
    destination.write_text(json.dumps(report, indent=2) + '\n')
    try:
        report.update(_run(work, vm, producer, realize, seconds, composition, compiler))
    except BaseException:
        report.update(state='failed', error=traceback.format_exc())
        raise
    finally:
        report['finished'] = time.time()
        destination.write_text(json.dumps(report, indent=2) + '\n')
    return report
