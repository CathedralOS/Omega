"""Independent signed reflection enumeration and 70-digit Bragg oracle.

Run before any Omega execution. This intentionally uses sets of signed triples
and combinations, independent of the application's ordered triple/mask loops.
"""
import hashlib
import itertools
import json
from pathlib import Path
import mpmath as mp

ROOT = Path(__file__).resolve().parents[1]
mp.mp.dps = 70

def reflections(kind):
    groups = {}
    for triple in itertools.product(range(-8, 9), repeat=3):
        squared = sum(value * value for value in triple)
        if squared == 0:
            continue
        if kind == 'I' and sum(triple) % 2:
            continue
        if kind == 'F' and len({value % 2 for value in triple}) != 1:
            continue
        groups.setdefault(squared, set()).add(tuple(sorted(map(abs, triple), reverse=True)))
    return [(squared, sorted(groups[squared])) for squared in sorted(groups)[:8]]

LINES = {kind: reflections(kind) for kind in ('P', 'I', 'F')}

def oracle(text):
    fields = text.split()
    wavelength, count, tolerance = mp.mpf(fields[0]), int(fields[1]), mp.mpf(fields[2])
    angles = list(map(mp.mpf, fields[3:]))
    assert len(angles) == count and 3 <= count <= 6
    assert mp.mpf('.5') <= wavelength <= mp.mpf('2.5') and 0 < tolerance <= 1
    assert all(0 < angle < 180 for angle in angles)
    assert all(left < right for left, right in zip(angles, angles[1:]))
    vectors = [(2 * mp.sin(angle * mp.pi / 360) / wavelength) ** 2 for angle in angles]
    hypotheses = []
    for kind, lines in LINES.items():
        for ranks in itertools.combinations(range(8), count):
            missing = [rank for rank in range(ranks[-1] + 1) if rank not in ranks]
            if len(missing) > 2:
                continue
            indices = [lines[rank][0] for rank in ranks]
            scale = sum(n * vector for n, vector in zip(indices, vectors)) / sum(n*n for n in indices)
            lattice = 1 / mp.sqrt(scale)
            if not 2 <= lattice <= 10:
                continue
            sines = [wavelength * mp.sqrt(scale * n) / 2 for n in indices]
            if any(value >= 1 for value in sines):
                continue
            predictions = [360 * mp.asin(value) / mp.pi for value in sines]
            residuals = [prediction - observed for prediction, observed in zip(predictions, angles)]
            if any(abs(value) > tolerance for value in residuals):
                continue
            hypotheses.append(dict(kind=kind, indices=indices, lattice=str(lattice), scale=str(scale),
                predicted=list(map(str, predictions)), residual=list(map(str, residuals)),
                families=[lines[rank][1] for rank in ranks], missing=[lines[rank][0] for rank in missing]))
    return dict(exit=0 if hypotheses else 2, hypotheses=hypotheses)

def pattern(wavelength, lattice, indices, tolerance='.000001', offsets=None):
    angles = [360 * mp.asin(mp.mpf(wavelength) * mp.sqrt(n) / (2 * mp.mpf(lattice))) / mp.pi for n in indices]
    if offsets:
        angles = [angle + mp.mpf(offset) for angle, offset in zip(angles, offsets)]
    return f'{wavelength} {len(indices)} {tolerance}\n' + ' '.join(f'{float(angle):.12f}' for angle in angles) + '\n'

def freeze():
    valid = {
        'primitive': pattern('1.54', '4', [1,2,3,4,5,6]),
        'body': pattern('1.2', '4.2', [2,4,6,8]),
        'face': pattern('1.54', '5.6', [3,4,8,11,12,16]),
        'two_missing': pattern('1.54', '5.6', [4,11,12,16]),
        'ambiguous': pattern('1', '4', [1,2,3]),
        'degenerate_families': pattern('1', '4', [1,2,4,6,8,9]),
        'perturbed': pattern('1.2', '4.2', [2,4,6,8,10], '.04', ['.01','-.012','.008','-.006','.005']),
        'near_minimum': pattern('.5', '2.00001', [1,2,3,4]),
        'near_maximum': pattern('2.5', '9.99999', [3,4,8,11,12,16]),
        'incompatible': '1.54 3 .000001\n17 31 77\n',
        'three_missing': pattern('1', '4', [4,5,6]),
    }
    good = valid['ambiguous']
    invalid = dict(empty='', missing='1 3 .1\n20 30\n', extra=good+'1\n',
        wavelength_low=good.replace('1 3', '.49 3', 1), wavelength_high=good.replace('1 3', '2.51 3', 1),
        count_low='1 2 .1\n20 30\n', count_high='1 7 .1\n10 20 30 40 50 60 70\n',
        count_fraction='1 3.5 .1\n20 30 40\n', angle_zero='1 3 .1\n0 20 30\n',
        angle_negative='1 3 .1\n-1 20 30\n', angle_180='1 3 .1\n20 30 180\n',
        duplicate='1 3 .1\n20 20 30\n', unordered='1 3 .1\n30 20 40\n',
        tolerance_zero=good.replace('.000001','0'), tolerance_negative=good.replace('.000001','-.1'),
        tolerance_high=good.replace('.000001','1.1'), nan=good.replace('.000001','nan'),
        infinity=good.replace('.000001','inf'), junk=good+'x\n', overlong=' '*2048)
    cases = {}
    (ROOT/'data').mkdir(exist_ok=True)
    for name, text in {**valid, **invalid}.items():
        payload = text.encode()
        (ROOT/'data'/f'{name}.txt').write_bytes(payload)
        expected = oracle(text) if name in valid else dict(exit=64, hypotheses=[])
        cases[name] = dict(input_sha256=hashlib.sha256(payload).hexdigest(), **expected)
    frozen = dict(precision_digits=70, lines=LINES, cases=cases)
    payload = (json.dumps(frozen, indent=2, sort_keys=True)+'\n').encode()
    (ROOT/'validation/oracle.json').write_bytes(payload)
    (ROOT/'validation/oracle.sha256').write_text(hashlib.sha256(payload).hexdigest()+'\n')
    print(hashlib.sha256(payload).hexdigest())
    for name, case in cases.items():
        print(name, case['exit'], len(case['hypotheses']))

if __name__ == '__main__':
    freeze()
