"""Independent Decimal oracle. Freeze before any Omega check or execution.

Uses Decimal.exp at 70 digits, a 4096-cell independent bracket scan and
240-step interval refinement. It does not call or translate the Omega exp.
The coarse 64-cell diagnostic model independently predicts budget exhaustion;
the high precision root is always calculated separately.
"""
import hashlib
import json
from decimal import Decimal as D, localcontext
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOLERANCES = {
    'voltage_absolute_V': '2e-9', 'voltage_relative': '2e-8',
    'current_absolute_A': '5e-12', 'current_relative': '3e-8',
    'reported_residual_absolute_V': '2e-9', 'residual_relative': '2e-6',
    'converged_residual_V': '1e-9', 'width_relative': '3e-8',
    'iterations_slack': 2,
}


def reference(values):
    start, stop, count, resistance, saturation, thermal, low, high, budget = values
    resistance = D(resistance)
    saturation = D(saturation) / D(10) ** 12
    thermal = D(thermal) / 1000
    low, high = D(low) / 1000, D(high) / 1000

    def current(voltage):
        return saturation * ((voltage / thermal).exp() - 1)

    def residual(voltage, source):
        return voltage + resistance * current(voltage) - source

    points = []
    for index in range(count):
        source = (D(start) + D(stop - start) * index / (count - 1)) / 1000
        # Independent fine-grid sign scan, including exact endpoints.
        previous = low
        previous_residual = residual(low, source)
        root = low if previous_residual == 0 else None
        fine_bracket = None
        for cell in range(1, 4097):
            if root is not None:
                break
            voltage = low + (high - low) * cell / 4096
            observed = residual(voltage, source)
            if observed == 0:
                root = voltage
                break
            if previous_residual < 0 < observed:
                fine_bracket = [previous, voltage]
                break
            previous, previous_residual = voltage, observed
        if fine_bracket:
            left, right = fine_bracket
            for _ in range(240):
                middle = (left + right) / 2
                if residual(middle, source) < 0:
                    left = middle
                else:
                    right = middle
            root = (left + right) / 2
        result = {'index': index, 'source_V': str(source)}
        if root is None:
            result.update(status='no_bracket', iterations=0, scan_evaluations=65)
        else:
            result.update(root_V=str(root), root_current_A=str(current(root)))
            left, left_residual = low, residual(low, source)
            if left_residual == 0:
                result.update(status='converged', iterations=0, scan_evaluations=1,
                              voltage_V=str(low), current_A=str(current(low)),
                              residual_V='0', width_V='0')
            else:
                for cell in range(1, 65):
                    right = low + (high - low) * cell / 64
                    observed = residual(right, source)
                    if observed == 0:
                        result.update(status='converged', iterations=0,
                                      scan_evaluations=cell + 1, voltage_V=str(right),
                                      current_A=str(current(right)), residual_V='0', width_V='0')
                        break
                    if left_residual < 0 < observed:
                        for iteration in range(1, budget + 1):
                            width = right - left
                            middle = (left + right) / 2
                            observed = residual(middle, source)
                            good = abs(observed) <= D('1e-10') or width <= D('1e-12')
                            if good or iteration == budget:
                                result.update(status='converged' if good else 'iteration_limit',
                                              iterations=iteration, scan_evaluations=cell + 1,
                                              voltage_V=str(middle), current_A=str(current(middle)),
                                              residual_V=str(observed), width_V=str(width))
                                break
                            if observed < 0:
                                left = middle
                            else:
                                right = middle
                        break
                    left, left_residual = right, observed
        points.append(result)
    return points


def freeze():
    target = ROOT / 'validation' / 'oracle.json'
    if target.exists():
        raise SystemExit('Refusing to overwrite frozen oracle; review changes explicitly.')
    cases = {}
    with localcontext() as context:
        context.prec = 70
        for fixture in sorted((ROOT / 'data').glob('*.stdin')):
            raw = fixture.read_bytes()
            case = {'stdin_sha256': hashlib.sha256(raw).hexdigest()}
            if fixture.stem.startswith('invalid_'):
                ranges = {'invalid_resistance', 'invalid_thermal', 'invalid_bracket',
                          'invalid_count', 'invalid_budget'}
                case['exit'] = 65 if fixture.stem in ranges else 64
                case['error'] = 'range' if case['exit'] == 65 else 'syntax'
            else:
                case['exit'] = 0
                case['points'] = reference(list(map(int, raw.split())))
            cases[fixture.stem] = case
    target.write_text(json.dumps({'model': 'Shockley-series-resistor-v1',
                                 'precision_digits': 70, 'fine_scan_cells': 4096,
                                 'root_refinement_steps': 240,
                                 'tolerances': TOLERANCES, 'cases': cases}, indent=2) + '\n')
    print(f'Frozen {len(cases)} cases: sha256={hashlib.sha256(target.read_bytes()).hexdigest()}')


if __name__ == '__main__':
    freeze()
