"""Byte observations for the public Epsilon-written Omega lexer."""
import struct
from pathlib import Path


def complete(cursor):
    return bytes((1, 0, 0, cursor, cursor, cursor, 0, 0, 0, 0))


def token(tag, start, end, cursor=None, decoded=0):
    return bytes((1, 1, 0, start, end, end if cursor is None else cursor, decoded, 0, 0, tag))


def rejection(tag, start, low, high, decoded=0):
    return bytes((0, 0, 0, start, start, start, decoded, low, high, tag))


def lexical_cases():
    # The first observation is one public scan at the supplied cursor; the
    # second is whole-view validation. Both run on the very same bytes.
    cases = []

    def accepted(name, source, cursor, selected):
        cases.append((name, source, cursor, selected, complete(len(source))))

    def refused(name, source, cursor, selected, whole=None):
        cases.append((name, source, cursor, selected, selected if whole is None else whole))

    accepted('empty', b'', 0, complete(0))
    accepted('end', b'if', 2, complete(2))
    accepted('identifier', b'ifx;', 0, token(1, 0, 3))
    accepted('keyword', b'if x', 0, token(5, 0, 2))
    accepted('integer', b'0xFe', 0, token(2, 0, 4))
    accepted('exponent', b'1e+9', 0, token(3, 0, 4))
    accepted('punctuation', b'if -> x', 3, token(6, 3, 5))
    accepted('whitespace', b' \t\r\nq', 0, token(7, 0, 4))
    accepted('line-comment', b'//x\nz', 0, token(8, 0, 3, cursor=4))
    accepted('nested-comment', b'/*a/*b*/c*/', 0, token(9, 0, 11))
    accepted('escaped-string', b'"A\\x46\\n"', 0, token(4, 0, 9, decoded=3))
    accepted('utf8-string', b'"\xc3\xa9"', 0, token(4, 0, 4, decoded=2))
    refused('invalid-utf8-before-token-query', b'if x\xff', 0, rejection(1, 0, 4, 5))
    refused('valid-utf8-outside-string', b'\xc3\xa9', 0, rejection(2, 0, 0, 2))
    refused('unfinished-comment', b'/*/*/', 0, rejection(3, 0, 0, 5))
    refused('unfinished-string', b'"a', 0, rejection(4, 0, 0, 2, decoded=1))
    refused('unfinished-escape', b'"\\', 0, rejection(5, 0, 0, 2))
    refused('unknown-escape', b'"\\q"', 0, rejection(6, 0, 1, 3))
    refused('short-hex', b'"\\x', 0, rejection(7, 0, 1, 3))
    refused('bad-first-hex', b'"\\xg0"', 0, rejection(8, 0, 3, 4))
    refused('bad-second-hex', b'"\\x0g"', 0, rejection(8, 0, 4, 5))
    refused('later-lexical-error', b'if "\\q"', 0, token(5, 0, 2), rejection(6, 3, 4, 6))
    accepted('cursor-outside-view', b'x', 2, bytes((0, 0, 1, 2, 2, 2, 0, 0, 0, 0)))
    return cases


def batch(cases):
    request = b''.join(bytes((len(source), cursor)) + source for _, source, cursor, _, _ in cases)
    expected = b'\0' + struct.pack('<i', 42) + b''.join(one + whole for _, _, _, one, whole in cases)
    return request, expected


def string_cases():
    cases = [
        ('empty-string', b'""', 0, token(4, 0, 2)),
        ('with-tail', b'"a"z', 0, token(4, 0, 3, decoded=1)),
        ('nonzero-cursor', b'x"b"', 1, token(4, 1, 4, decoded=1)),
        ('simple-escapes', br'"\n\r\t\0\\\""', 0, token(4, 0, 14, decoded=6)),
        ('hex-extremes', br'"\x00\xFf"', 0, token(4, 0, 10, decoded=2)),
        ('raw-utf8-bytes', b'"\xc3\xa9"', 0, token(4, 0, 4, decoded=2)),
        ('unfinished', b'"a', 0, rejection(4, 0, 0, 2, decoded=1)),
        ('unfinished-escape', b'"a\\', 0, rejection(5, 0, 0, 3, decoded=1)),
        ('unknown-escape', br'"\q"', 0, rejection(6, 0, 1, 3)),
        ('short-hex', br'"\x0', 0, rejection(7, 0, 1, 4)),
        ('invalid-high', br'"\xg0"', 0, rejection(8, 0, 3, 4)),
        ('invalid-low', br'"\x0g"', 0, rejection(8, 0, 4, 5)),
        ('unicode-escape', br'"\u{41}"', 0, rejection(2, 0, 1, 3)),
        ('quoted-newline', b'"a\nb"', 0, rejection(2, 0, 2, 3, decoded=1)),
        ('quoted-carriage-return', b'"\r"', 0, rejection(2, 0, 1, 2)),
        ('wrong-opener', b'x', 0, bytes((0, 0, 1, 0, 0, 0, 0, 0, 0, 0))),
        ('at-eof', b'x', 1, bytes((0, 0, 1, 1, 1, 1, 0, 0, 0, 0))),
    ]
    return [(label, source, offset, result, b'') for label, source, offset, result in cases]


def scan_source(root, fixtures, boundary, public, *, public_entry='lexer_public_main.epsilon'):
    base = root / 'bootstrap/omega/compiler'
    representations = (base / 'representations.epsilon').read_bytes()
    lexer = (base / 'source-to-tokens/scan.epsilon').read_bytes()
    utf8 = (base / 'request_and_utf8.epsilon').read_bytes()
    # Complete authored declarations; the lexer receiver contains a UTF-8
    # validation field even when the customer enters only its string stage.
    utf8_type = representations[representations.index(b'data OmegaUtf8Validation {'):
                                representations.index(b'data OmegaRequestEnvelope {')]
    token_types = representations[representations.index(b'data OmegaNumericBase {'):
                                 representations.index(b'data OmegaRelativeSpan {')]
    lexical = (base / 'source-to-tokens/byte_classes.epsilon').read_bytes()
    if public:
        implementation = utf8[utf8.index(b'machine OmegaUtf8Validation::validate('):] + lexical + lexer
        customer = fixtures / public_entry
    else:
        implementation = (lexical + lexer[:lexer.index(b'machine OmegaLexScan::span_equals(')]
                          + lexer[lexer.index(b'machine OmegaLexScan::scan_string('):
                                  lexer.index(b'machine OmegaLexScan::consume_digits(')])
        customer = fixtures / 'lexer_string_main.epsilon'
    return utf8_type + token_types + implementation + boundary + (fixtures / 'lexer_scan_main.epsilon').read_bytes() + customer.read_bytes()


def public_scan_source(root, entry):
    fixtures = root / 'tests/bootstrap/independent/epsilon'
    boundary = (fixtures / 'console.epsilon').read_bytes()
    return scan_source(root, fixtures, boundary, public=True, public_entry=entry)


def public_scan_case(root):
    source = public_scan_source(root, 'lexer_public_main.epsilon')
    payload, expected = batch(lexical_cases())
    return source, payload, expected


def punctuation_case(root):
    source = public_scan_source(root, 'lexer_punctuation_main.epsilon')
    spellings = br"""& && ' -> * ^ : :: , . .. ..= = == ! != > >= >> # { [ (
                     < <= << - % | || + += -= *= /= %= } ] ) ; / ~ @ \ $""".split()
    request = bytearray()
    expected = bytearray(b'\0' + struct.pack('<i', 42))
    for spelling in spellings:
        for text, offset in ((spelling, 0), (b'x' + spelling + b'q', 1)):
            request.extend(bytes((len(text), offset)) + text)
            expected.extend(bytes([len(spelling)]) + spelling + b'\n')
    return source, bytes(request), bytes(expected)


def public_scan_wrong_offset_case(root):
    source, payload, _ = public_scan_case(root)
    call = b'self.scanner.scan_at(source, offset)'
    if source.count(call) != 1:
        raise ValueError('expected one public scan call in the lexer customer')
    offset = source.index(call)
    invalid = source.replace(call, b'self.scanner.scan_at(source, source)')
    return invalid, payload, struct.pack('<BBI', 2, 18, offset)


def run(work, composition, boundary, compiler, observe, compile_delta, reference, identity, alpha_program, check_alpha):
    """Use the same Gamma receipt and input bytes for both realizations."""
    fixtures = Path(__file__).parent
    compiler_receipt = (work / 'compiler.gamma').read_bytes()
    alpha_host, compiler_tape = alpha_program('scan-compiler', compiler_receipt)
    results = {'compiler_receipt': identity(compiler_receipt), 'alpha_compiler_tape': identity(compiler_tape)}
    for name, cases in [('scan-string', string_cases()), ('scan-public', lexical_cases())]:
        source = scan_source(composition.ROOT, fixtures, boundary, name == 'scan-public')
        (work / f'{name}.epsilon').write_bytes(source)
        observed = observe(compiler, source)
        if name == 'scan-string':
            alpha_observed = observe(alpha_host, source)
            (work / f'{name}.alpha-compiler-output').write_bytes(alpha_observed)
            assert alpha_observed == observed, (name, 'Epsilon compiler host mismatch')
        generated, runtime = composition.decode_module(observed)
        assert runtime == 'storage'
        (work / f'{name}.observation').write_bytes(observed)
        target = composition.program_source(generated, runtime=runtime)
        native = compile_delta(target, name)
        receipt = (work / f'{name}.gamma').read_bytes()
        payload, expected = batch(cases)
        (work / f'{name}.input').write_bytes(payload)
        (work / f'{name}.expected').write_bytes(expected)
        original = reference(source, payload, f'{name}-evaluator')
        actual = observe(native, payload)
        (work / f'{name}.native.output').write_bytes(actual)
        assert original == expected and actual == expected, (name, original.hex(), actual.hex())
        report = {'cases': [row[0] for row in cases], 'source': identity(source),
                  'compiler_output': identity(observed), 'generated': identity(generated),
                  'delta': identity(target), 'receipt': identity(receipt), 'native': identity(native.read_bytes()),
                  'input': identity(payload), 'output': identity(expected), 'epsilon_exit': 42}
        report.update(check_alpha(name, payload, expected))
        if name == 'scan-string':
            call = b'token_tag(self.scanner.token_kind)'
            assert source.count(call) == 1
            bad = source.replace(call, b'token_tag(self.scanner.diagnostic_code)')
            offset = source.index(call)
            refused = observe(compiler, bad)
            assert observe(alpha_host, bad) == refused
            (work / f'{name}-wrong-owner.epsilon').write_bytes(bad)
            (work / f'{name}-wrong-owner.output').write_bytes(refused)
            assert refused == b'rejected 18 ' + str(offset).encode(), refused
            assert reference(bad, b'', f'{name}-wrong-owner-evaluator') == b'\x02\x12' + struct.pack('<I', offset)
            report['wrong_owner'] = {'source': identity(bad), 'offset': offset, 'observation': identity(refused)}
        results[name] = report
    return results
