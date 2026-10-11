"""Check captured Omega output against the pre-execution immutable oracle."""
import csv
import hashlib
import json
import sys
from decimal import Decimal as D, localcontext
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def near(actual, expected, absolute, relative):
    assert abs(actual - expected) <= D(absolute) + abs(expected) * D(relative), (actual, expected)


def check(case_name, output, exit_code):
    oracle = json.loads((ROOT / 'validation/oracle.json').read_text())
    case = oracle['cases'][case_name]
    fixture = ROOT / 'data' / (case_name + '.stdin')
    assert hashlib.sha256(fixture.read_bytes()).hexdigest() == case['stdin_sha256']
    assert exit_code == case['exit'], (exit_code, case['exit'])
    text = Path(output).read_text()
    if case['exit']:
        assert text.strip() == ('ERROR range: invalid sweep or Shockley model parameters'
                               if case['error'] == 'range' else
                               'ERROR syntax: expected nine integers, maximum 160 bytes'), text
        return
    rows = list(csv.reader(text.splitlines()))
    assert rows[0] == ['point', 'status', 'source_V', 'diode_V', 'current_A',
                       'residual_V', 'width_V', 'iterations', 'evaluations']
    assert len(rows) == len(case['points']) + 2
    tolerance = oracle['tolerances']
    solved = 0
    inputs = list(map(int, fixture.read_bytes().split()))
    resistance, saturation, thermal = D(inputs[3]), D(inputs[4]) / D(10) ** 12, D(inputs[5]) / 1000
    for row, expected in zip(rows[1:-1], case['points']):
        assert len(row) == 9
        assert int(row[0]) == expected['index'] and row[1] == expected['status'], row
        source = D(row[2])
        near(source, D(expected['source_V']), '2e-9', '2e-8')
        iterations, evaluations = map(int, row[7:])
        if expected['status'] == 'no_bracket':
            assert row[3:7] == ['NA'] * 4 and iterations == 0 and evaluations == 65
            continue
        voltage, current, residual, width = map(D, row[3:7])
        assert D(inputs[6]) / 1000 <= voltage <= D(inputs[7]) / 1000
        near(current, saturation * ((voltage / thermal).exp() - 1),
             tolerance['current_absolute_A'], '5e-7')
        recomputed = voltage + resistance * current - source
        # Reporting is rounded to nine digits independently in each column.
        near(residual, recomputed, '2e-7', tolerance['residual_relative'])
        assert 0 <= iterations <= inputs[8]
        assert evaluations == expected['scan_evaluations'] + iterations
        if iterations:
            expected_width = (D(inputs[7] - inputs[6]) / 1000 / 64) / D(2) ** (iterations - 1)
            near(width, expected_width, '1e-20', tolerance['width_relative'])
        else:
            assert width == 0
        if expected['status'] == 'converged':
            solved += 1
            near(voltage, D(expected['root_V']), tolerance['voltage_absolute_V'],
                 tolerance['voltage_relative'])
            near(current, D(expected['root_current_A']), tolerance['current_absolute_A'],
                 tolerance['current_relative'])
            assert abs(residual) <= D(tolerance['converged_residual_V'])
            assert abs(iterations - expected['iterations']) <= tolerance['iterations_slack']
        else:
            assert iterations == inputs[8]
            near(voltage, D(expected['voltage_V']), '2e-9', '2e-8')
            near(residual, D(expected['residual_V']),
                 tolerance['reported_residual_absolute_V'], tolerance['residual_relative'])
    assert rows[-1] == ['summary', str(solved), str(len(case['points']) - solved)]


if __name__ == '__main__':
    with localcontext() as context:
        context.prec = 70
        check(sys.argv[1], sys.argv[2], int(sys.argv[3]))
    print('verified ' + sys.argv[1])
