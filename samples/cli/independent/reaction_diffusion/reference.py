"""Independent integer oracle. Run --freeze only before Omega execution.

Replicates the decimal-thousandth explicit Euler reaction-diffusion update with
truncating division exactly: the same operator at full step, plus a shadow
trajectory of two half steps whose largest cell disagreement is the reported
continuum error. Mass accounting keeps reaction and diffusion contributions
separate; every emitted row satisfies mass = previous + reaction + diffusion.
"""
from fractions import Fraction
from pathlib import Path
import hashlib
import json
import sys
ROOT = Path(__file__).resolve().parent
HEADER = 'step,mass,reaction,diffusion,continuum_error\n'
BOUND = 1000000

def tdiv(numerator, denominator):
    """Truncating (round-toward-zero) integer division, matching Omega `/`."""
    quotient = abs(numerator) // abs(denominator)
    return -quotient if (numerator < 0) != (denominator < 0) else quotient

def sweep(field, cells, mode, lam, rate, capacity, divisor):
    """One Jacobi sweep; returns (next_field, reaction_mass, diffusion_mass)."""
    result = []
    reaction = 0
    diffusion = 0
    for index in range(cells):
        left = field[index - 1] if index > 0 else (field[cells - 1] if mode == 1 else field[0])
        right = field[index + 1] if index < cells - 1 else (field[0] if mode == 1 else field[cells - 1])
        center = field[index]
        laplacian = left + right - 2 * center
        diffusion_term = tdiv(lam * laplacian, divisor)
        reaction_term = tdiv(tdiv(rate * center * (capacity - center), divisor), capacity)
        result.append(center + diffusion_term + reaction_term)
        reaction += reaction_term
        diffusion += diffusion_term
    return result, reaction, diffusion

def advance(field, shadow, cells, mode, lam, rate, capacity):
    field, reaction, diffusion = sweep(field, cells, mode, lam, rate, capacity, 1000)
    shadow, _, _ = sweep(shadow, cells, mode, lam, rate, capacity, 2000)
    shadow, _, _ = sweep(shadow, cells, mode, lam, rate, capacity, 2000)
    error = max(abs(a - b) for a, b in zip(field, shadow))
    return field, shadow, reaction, diffusion, error

def oracle(values):
    cells, steps, mode, lam, rate, capacity = values[:6]
    supplied = values[6:]
    if not (3 <= cells <= 32 and 0 <= steps <= 64 and mode in (0, 1)
            and 0 <= lam <= 500 and 0 <= rate <= 1000 and 1 <= capacity <= BOUND):
        return 2, 'ERROR invalid input\n'
    if len(supplied) != cells or not all(0 <= value <= BOUND for value in supplied):
        return 2, 'ERROR invalid input\n'
    field = list(supplied)
    shadow = list(supplied)
    rows = [f'0,{sum(field)},0,0,0']
    for step in range(steps):
        before = sum(field)
        field, shadow, reaction, diffusion, error = advance(field, shadow, cells, mode, lam, rate, capacity)
        mass = sum(field)
        assert mass == before + reaction + diffusion, 'reaction-inclusive mass ledger must balance'
        if not all(0 <= value <= BOUND for value in field):
            return 3, HEADER + '\n'.join(rows) + '\nERROR infeasible state\n'
        rows.append(f'{step + 1},{mass},{reaction},{diffusion},{error}')
    return 0, HEADER + '\n'.join(rows) + '\nfield,' + ','.join(map(str, field)) + '\n'

CASES = {
    # Reflecting (Neumann) boundary: mass is transported, never gained or lost.
    'reflecting_pulse': [8, 6, 0, 200, 0, 1000000, 0, 0, 300000, 500000, 300000, 0, 0, 0],
    # Periodic boundary wraps both ends into one ring.
    'periodic_pulse': [8, 6, 1, 200, 0, 1000000, 0, 0, 300000, 500000, 300000, 0, 0, 0],
    # Pure logistic reaction with no spatial coupling.
    'reaction_growth': [6, 8, 0, 0, 500, 500000, 100000, 120000, 140000, 160000, 180000, 200000],
    # Fisher-KPP transport toward capacity under a reflecting wall.
    'fisher_reflecting': [8, 10, 0, 250, 400, 800000, 0, 0, 900000, 900000, 100000, 0, 50000, 0],
    'fisher_periodic': [8, 10, 1, 250, 400, 800000, 0, 0, 900000, 900000, 100000, 0, 50000, 0],
    'zero_steps': [8, 0, 0, 200, 300, 800000, 100000, 100000, 100000, 100000, 100000, 100000, 100000, 100000],
    'minimum_cells': [3, 6, 1, 300, 200, 600000, 0, 300000, 600000],
    'maximum_cells': [32, 4, 0, 150, 100, 900000] + [i * 10000 for i in range(32)],
    'stability_edge': [8, 6, 1, 500, 0, 1000000, 0, 0, 0, 1000000, 0, 0, 0, 0],
    'extinct': [6, 4, 0, 300, 800, 500000, 0, 0, 0, 0, 0, 0],
    'saturated': [6, 4, 0, 300, 800, 500000, 500000, 500000, 500000, 500000, 500000, 500000],
    'over_capacity': [6, 6, 0, 100, 600, 400000, 480000, 500000, 450000, 470000, 460000, 490000],
    # Extreme reaction over a trivial capacity drives cells below zero.
    'infeasible': [4, 2, 0, 0, 1000, 1, 1000000, 1000000, 1000000, 1000000],
    'bad_mode': [6, 2, 2, 200, 300, 500000, 100000, 100000, 100000, 100000, 100000, 100000],
    'bad_cells': [33, 2, 0, 200, 300, 500000, 100000, 100000, 100000, 100000, 100000, 100000],
    'bad_lam': [6, 2, 0, 501, 300, 500000, 100000, 100000, 100000, 100000, 100000, 100000],
    'bad_rate': [6, 2, 0, 200, 1001, 500000, 100000, 100000, 100000, 100000, 100000, 100000],
    'bad_capacity': [6, 2, 0, 200, 300, 0, 100000, 100000, 100000, 100000, 100000, 100000],
    'negative_value': [6, 2, 0, 200, 300, 500000, 100000, -5, 100000, 100000, 100000, 100000],
    'value_overflow': [6, 2, 0, 200, 300, 500000, 100000, 1000001, 100000, 100000, 100000, 100000],
    'short_field': [8, 2, 0, 200, 300, 500000, 100000, 100000, 100000, 100000],
}

def freeze():
    manifest = {'tolerance': 0,
        'arithmetic': 'decimal thousandths; truncating division; Jacobi explicit Euler; dual half-step shadow continuum error',
        'cases': {}}
    for name, values in CASES.items():
        content = ''.join(f'{value}\n' for value in values)
        status, output = oracle(values)
        (ROOT/'fixtures'/f'{name}.input').write_text(content)
        (ROOT/'fixtures'/f'{name}.stdout').write_text(output)
        manifest['cases'][name] = {'exit': status,
            'input_sha256': hashlib.sha256(content.encode()).hexdigest(),
            'stdout_sha256': hashlib.sha256(output.encode()).hexdigest()}
    for name, content in {'malformed': '6\n2x\n', 'missing': '6\n2\n',
                          'overlong': '12345678901\n', 'empty': '\n',
                          'trailing': ''.join(f'{v}\n' for v in CASES['reaction_growth']) + '0\n'}.items():
        output = 'ERROR invalid input\n'
        (ROOT/'fixtures'/f'{name}.input').write_text(content)
        (ROOT/'fixtures'/f'{name}.stdout').write_text(output)
        manifest['cases'][name] = {'exit': 2,
            'input_sha256': hashlib.sha256(content.encode()).hexdigest(),
            'stdout_sha256': hashlib.sha256(output.encode()).hexdigest()}
    (ROOT/'fixtures/oracle.json').write_text(json.dumps(manifest, indent=2) + '\n')

if __name__ == '__main__':
    if sys.argv[1:] != ['--freeze']:
        raise SystemExit('usage: reference.py --freeze (before execution only)')
    freeze()
