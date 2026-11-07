#!/usr/bin/env python3
"""Independent byte-format oracle. Freeze before compiler or application runs."""
import json
from pathlib import Path
import random
import struct

ROOT = Path(__file__).resolve().parent

def encode(values, tolerance):
    step = 2 * tolerance + 1
    reconstruction = [min(4095, step * ((value + tolerance) // step)) for value in values]
    output = bytearray([83, 67, 1, len(values), tolerance])
    previous = 0
    for value in reconstruction:
        delta = value - previous
        output.extend([delta + 127] if -127 <= delta <= 127 else [255, value % 256, value // 256])
        previous = value
    maximum = max(abs(a - b) for a, b in zip(values, reconstruction))
    assert maximum <= tolerance
    assert len(output) <= 5 + 3 * len(values)
    return bytes(output), reconstruction, maximum

def decode(encoded):
    if not 6 <= len(encoded) <= 53 or encoded[:3] != b'SC\x01':
        raise ValueError('header')
    count, tolerance = encoded[3:5]
    if not 1 <= count <= 16 or not 0 <= tolerance <= 7:
        raise ValueError('bounds')
    cursor, previous, values = 5, 0, []
    for _ in range(count):
        if cursor >= len(encoded):
            raise ValueError('truncated tag')
        tag = encoded[cursor]
        cursor += 1
        if tag == 255:
            if cursor + 2 > len(encoded):
                raise ValueError('truncated absolute')
            value = int.from_bytes(encoded[cursor:cursor+2], 'little')
            cursor += 2
            if -127 <= value - previous <= 127:
                raise ValueError('unnecessary escape')
        else:
            value = previous + tag - 127
        if not 0 <= value <= 4095:
            raise ValueError('reconstruction bounds')
        values.append(value)
        previous = value
    if cursor != len(encoded):
        raise ValueError('trailing bytes')
    return values

def report(encoded, values, tolerance, maximum=None):
    result = 'bytes: ' + ' '.join(map(str, encoded)) + '\n'
    result += 'reconstruction: ' + ' '.join(map(str, values)) + '\n'
    result += f'raw_bytes: {2*len(values)}\nencoded_bytes: {len(encoded)}\nworst_bytes: {5+3*len(values)}\ntolerance_units: {tolerance}\n'
    if maximum is None:
        result += 'certificate: decode only; originals unavailable\n'
    else:
        result += f'max_error_units: {maximum}\ncertificate: verified against original records\n'
    return result

def freeze():
    cases = []
    def add(name, request, stdout, exit_code):
        path = ROOT / 'data' / (name + '.bin')
        path.write_bytes(request)
        expected = ROOT / 'data' / (name + '.stdout')
        expected.write_text(stdout)
        cases.append(dict(name=name, input=path.name, stdout=expected.name, exit=exit_code))
    fixtures = [
        ('exact', [0,1,127,254,127,0,4095,3968], 0),
        ('smooth', [1600,1601,1602,1605,1607,1608,1609,1610,1612,1613,1615,1616,1617,1618,1620,1621], 2),
        ('abrupt', [0,4095,0,4095,1,4094,2048,2047], 3),
        ('grid_edges', [0,1,2,3,4,7,8,14,15,16,4087,4090,4094,4095], 7),
        ('delta_edges', [127,254,127,255,127,0], 0),
        ('single', [4095], 7),
        ('worst_size', [4095,0]*8, 0),
    ]
    random_source = random.Random(92026)
    fixtures += [(f'seeded_{e}', [random_source.randrange(4096) for _ in range(16)], e) for e in (0,1,4,7)]
    specifications = []
    for name, values, tolerance in fixtures:
        encoded, reconstruction, maximum = encode(values, tolerance)
        assert decode(encoded) == reconstruction
        add(name, bytes([0,len(values),tolerance]) + b''.join(struct.pack('<H', v) for v in values), report(encoded,reconstruction,tolerance,maximum),0)
        add(name+'_decode', b'\x01'+encoded, report(encoded,reconstruction,tolerance),0)
        specifications.append(dict(name=name, values=values, tolerance=tolerance, encoded=list(encoded), reconstruction=reconstruction, max_error=maximum))
    # Hand-calculated exact decoder fixture is independent of encode().
    manual = bytes([83,67,1,4,0,254,0,255,0,16,127])  # illegal absolute 4096
    malformed = {
        'bad_magic': bytes([0,67,1,1,0,127]),
        'bad_version': bytes([83,67,2,1,0,127]),
        'bad_count': bytes([83,67,1,17,0,127]),
        'zero_count': bytes([83,67,1,0,0,127]),
        'bad_tolerance': bytes([83,67,1,1,8,127]),
        'truncated_tag': bytes([83,67,1,2,0,127]),
        'truncated_escape': bytes([83,67,1,1,0,255,0]),
        'absolute_bounds': manual,
        'delta_underflow': bytes([83,67,1,1,0,0]),
        'delta_overflow': bytes([83,67,1,2,0,255,255,15,128]),
        'unnecessary_escape': bytes([83,67,1,1,0,255,1,0]),
        'trailing_byte': bytes([83,67,1,1,0,127,127]),
    }
    for name, encoded in malformed.items():
        try:
            decode(encoded)
        except ValueError:
            pass
        else:
            raise AssertionError(name)
        add(name,b'\x01'+encoded,'error: malformed sensor stream\n',12)
    invalid = {
        'empty_input': b'', 'unknown_mode': b'\x02',
        'too_many_input_bytes': bytes(57),
        'empty_records': bytes([0,0,0]),
        'excess_records': bytes([0,17,0])+bytes(34),
        'invalid_tolerance': bytes([0,1,8,0,0]),
        'record_bounds': bytes([0,1,0,0,16]),
        'truncated_records': bytes([0,2,0,1,0]),
        'extra_record_bytes': bytes([0,1,0,1,0,0]),
        'too_short_stream': b'\x01SC\x01\x01\x00',
    }
    for name, request in invalid.items():
        add(name, request, 'error: input bounds or syntax\n',10)
    (ROOT/'data'/'cases.json').write_text(json.dumps(cases,indent=2)+'\n')
    (ROOT/'data'/'oracle.json').write_text(json.dumps(specifications,indent=2)+'\n')
    print(f'Frozen {len(cases)} exact output/exit cases; tolerance is integer E, no float epsilon.')

if __name__ == '__main__':
    freeze()
