"""Independent dense Haar matrix oracle, never the program's butterfly route."""
from decimal import Decimal, localcontext
from pathlib import Path
import json

ROOT = Path(__file__).resolve().parent

def oracle(text):
    with localcontext() as context:
        context.prec = 80
        tokens = text.split()
        size, threshold, levels = int(tokens[0]), Decimal(tokens[1]), int(tokens[2])
        samples = list(map(Decimal, tokens[3:]))
        assert len(samples) == size
        # Rows are defined directly by support/sign, from coarsest to finest.
        matrix = [[Decimal(1) / Decimal(size).sqrt()] * size]
        width = size
        while width >= 2:
            weight = Decimal(1) / Decimal(width).sqrt()
            for start in range(0, size, width):
                matrix.append([weight if start <= column < start + width // 2 else
                               -weight if start + width // 2 <= column < start + width else
                               Decimal(0) for column in range(size)])
            width //= 2
        coefficients = [sum(a * b for a, b in zip(row, samples)) for row in matrix]
        cutoff = size // (2 ** levels)
        retained = [((max(abs(value) - threshold, Decimal(0)) * (1 if value >= 0 else -1))
                     if index > 0 and index >= cutoff else value)
                    for index, value in enumerate(coefficients)]
        reconstruction = [sum(matrix[row][column] * retained[row] for row in range(size))
                          for column in range(size)]
        energy = sum(value * value for value in samples)
        assert abs(sum(value * value for value in coefficients) - energy) < Decimal('1e-70') * max(1, energy)
        removed = energy - sum(value * value for value in retained)
        return dict(original=list(map(str, coefficients)), retained=list(map(str, retained)),
                    reconstruction=list(map(str, reconstruction)), input_energy=str(energy), removed_energy=str(removed))

def fixtures():
    cases = []
    def add(name, values, threshold, levels):
        text = f'{len(values)} {threshold} {levels}\n' + ' '.join(map(str, values)) + '\n'
        cases.append(dict(name=name, input=text, **oracle(text)))
    for size in (8, 16, 32, 64):
        depth = size.bit_length() - 1
        add(f'constant{size}', ['-1.25'] * size, '0.3', depth)
        add(f'impulse{size}', [1] + [0] * (size - 1), '0.1', 1)
        add(f'roundtrip{size}', [str(Decimal((i * 17) % 23 - 11) / 8) for i in range(size)], 0, depth)
        add(f'step{size}', [-2] * (size // 2) + [3] * (size // 2), '0.7', depth)
        add(f'full_removal{size}', [(-1) ** i * (i + 1) for i in range(size)], 1000, depth)
        add(f'no_levels{size}', [(-1) ** i * (i + 1) for i in range(size)], 1000, 0)
    add('ties', [2, 2, -2, -2, -2, -2, 2, 2], 4, 3)
    add('partial_levels', [3, -1, 2, -4, 1, 0, 2, -1, 4, 2, 1, 3, -2, 0, 1, -3], '0.25', 2)
    add('fractional', ['0.000000001', '-0.012345678', '1.125', '-2.5', '0.25', '-0.25', '3.75', '-0.125'], '0.012345678', 3)
    add('bounds', [1000, -1000] * 32, 1000, 6)
    return cases

if __name__ == '__main__':
    (ROOT / 'fixtures.json').write_text(json.dumps(fixtures(), indent=2) + '\n')
    (ROOT / 'input.txt').write_text(fixtures()[25]['input'])
