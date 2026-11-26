"""Check the standard byte-read result, including the signed payload's two bounds."""
import runpy
import struct


def byte_input_cases(root):
    encode = runpy.run_path(str(root / 'source/application/construction/construct.py'))['encode']
    system = (root / 'source/library/system/console.omg').read_bytes()
    start = system.index(b'public data ByteRead')
    declaration = system[start:system.index(b'\n}', start) + 2] + b'\n'
    decoder = b'''
machine decode(word:i32)->ByteRead crashes Trap {
    transition word == -1 {true -> eof(), _ -> check(word)}
    machine eof()->ByteRead {ByteRead::Eof}
    machine check(word:i32)->ByteRead {
        transition word >= 0 && word <= 255 {true -> byte(word), _ -> invalid()}
    }
    machine byte(word:i32)->ByteRead requires word >= 0 && word <= 255 {
        ByteRead::Byte {value:word}
    }
    machine invalid()->ByteRead {crash Trap;}
}
'''
    entry = b'''
public machine main()->u8 crashes Trap {
    let result:ByteRead=decode(INPUT);
    transition result {ByteRead::Eof -> eof(), ByteRead::Byte {value} -> byte(value)}
    machine eof()->u8 {EOF}
    machine byte(word:i32)->u8 requires word >= 0 && word <= 255 {
        transition word == EXPECT {true -> 42, _ -> 0}
    }
}
'''

    def trial(name, source, expected):
        wire, _, _, units = encode([('app', 0)], [], [(0, source)], 0, b'main', paths=['main.omg'])
        if isinstance(expected, tuple):
            code, token = expected
            expected = struct.pack('<BII', 0, code, units[0][1] + source.index(token))
        return name, wire, expected

    def program(value):
        selected = entry.replace(b'INPUT', str(value).encode()).replace(b'EXPECT', str(max(value, 0)).encode())
        return declaration + decoder + selected.replace(b'EOF', b'42' if value == -1 else b'0')

    for name, value, expected in [
        ('eof', -1, 42), ('zero', 0, 42), ('high', 255, 42),
        ('below', -2, 'trap'), ('above', 256, 'trap'),
    ]:
        yield trial('byte-input-' + name, program(value), expected)

    source = program(42)
    renamed = source.replace(b'ByteRead', b'Result').replace(b'::Byte', b'::Item').replace(b'case Byte(', b'case Item(')
    yield trial('byte-input-renamed', renamed.replace(b'value', b'number'), 42)
    for name, old, new in [
        ('guard-lower', b'word >= 0 && word <= 255 {true', b'word <= 255 {true'),
        ('guard-upper', b'word >= 0 && word <= 255 {true', b'word >= 0 {true'),
        ('requires-lower', b'->ByteRead requires word >= 0 && word <= 255', b'->ByteRead requires word <= 255'),
        ('requires-upper', b'->ByteRead requires word >= 0 && word <= 255', b'->ByteRead requires word >= 0'),
    ]:
        assert source.count(old) == 1
        yield trial('byte-input-' + name, source.replace(old, new), (2, b'machine decode'))

    for value in (b'-1', b'256'):
        source = declaration + b'public machine main(){let r:ByteRead=ByteRead::Byte{value:' + value + b'};}'
        yield trial('byte-input-invalid-' + value.decode(), source, (1, value))
    for name, changed in [
        ('owner', declaration.replace(b'self in ByteRead::Eof', b'self in Other::Eof') + b'data Other {case Eof;}'),
        ('omitted-case', declaration.replace(b'case Eof;', b'case More;case Eof;')),
        ('wrong-field', declaration.replace(b'self.value >= 0', b'self.other >= 0')
         .replace(b'Byte(value: i32)', b'Byte(value: i32, other: i32)')),
    ]:
        yield trial('byte-input-' + name, changed + b'public machine main(){}', (2, b'public data'))
    source = declaration + b'public machine main(){let r:ByteRead=ByteRead::Eof;let duplicate:ByteRead=r;}'
    yield trial('byte-input-affine-copy', source, (2, b'public machine'))
    # With the range removed from the sum, matching its case gives only a plain i32.
    unrestricted = declaration.replace(b' && (self.value >= 0 && self.value <= 255)', b'')
    yield trial('byte-input-unbounded-binding', program(42).replace(declaration, unrestricted),
                (2, b'public machine main'))

    # The caller and both suppliers can give different meanings to ByteRead.
    # Each predicate must resolve under its declaring package and its file.
    caller = b'use api\\ByteRead;\n' + decoder + entry.replace(b'INPUT', b'255').replace(b'EXPECT', b'255').replace(b'EOF', b'0')
    tighter = declaration.replace(b'<= 255', b'<= 10')
    for name, app, broad, expected in [
        ('forward', caller, declaration, 42),
        ('forward-wrong-owner', caller.replace(b'use api', b'use strict'), declaration, (0, b'machine decode', 2)),
        ('unreachable-invalid-case', caller, declaration.replace(b'self in ByteRead::Eof', b'self in ByteRead::Missing'),
         (1, b'public data', 2)),
        ('bad-guard', caller.replace(b'word >= 0 && word <= 255 {true', b'word <= 255 {true'), declaration,
         (0, b'machine decode', 2)),
        ('forbidden-crash', caller.replace(b'public machine main()->u8 crashes Trap', b'public machine main()->u8 crashes none'),
         declaration, (0, b'public machine main', 2)),
    ]:
        inputs = [(0, app), (1, broad), (2, tighter)]
        wire, _, _, rows = encode([('app', 0), ('broad', 0), ('other', 0)], [(0, 1, 'api'), (0, 2, 'strict')],
                                  inputs, 0, b'main', paths=['main.omg', 'result.omg', 'strict.omg'])
        if isinstance(expected, tuple):
            unit, marker, code = expected
            expected = struct.pack('<BII', 0, code, rows[unit][1] + inputs[unit][1].index(marker))
        yield 'byte-input-files-' + name, wire, expected
