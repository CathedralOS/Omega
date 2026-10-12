"""Check captured Omega output against the pre-execution immutable oracle."""
import hashlib
import json
from decimal import Decimal as D, localcontext
from pathlib import Path

from reference import simulate

ROOT = Path(__file__).resolve().parent.parent


def near(actual, expected, absolute, relative):
    assert abs(actual - expected) <= D(absolute) + abs(expected) * D(relative), (actual, expected)


def emitted(text):
    """Decode the mantissa-exponent emission (`420000000e-9` means 0.42)."""
    text = text.strip()
    if text == '0':
        return D(0)
    mantissa, exponent = text.split('e')
    return D(mantissa) * D(10) ** int(exponent)


def check(case_name, output, exit_code):
    oracle = json.loads((ROOT / 'validation/oracle.json').read_text())
    case = oracle['cases'][case_name]
    fixture = ROOT / 'data' / (case_name + '.stdin')
    assert hashlib.sha256(fixture.read_bytes()).hexdigest() == case['stdin_sha256']
    assert exit_code == case['exit'], (exit_code, case['exit'])
    text = Path(output).read_text()
    if case['exit']:
        expected = ('ERROR range: invalid cell, load or discharge schedule parameters'
                    if case['error'] == 'range' else
                    'ERROR syntax: expected eight integers, maximum 160 bytes')
        assert text.strip() == expected, text
        return
    lines = [line for line in text.splitlines() if line]
    header = ['step', 'time_s', 'ocv_V', 'terminal_V', 'current_A',
              'delivered_mAh', 'delivered_Wh']
    assert lines[0].split(',') == header, lines[0]
    assert lines[-2].startswith('stop,'), lines[-2]
    assert lines[-1].startswith('summary,'), lines[-1]
    rows = [line.split(',') for line in lines[1:-2]]
    stop = lines[-2].split(',')
    summary = lines[-1].split(',')
    inputs = fixture.read_bytes().split()
    with localcontext() as context:
        context.prec = 70
        expected_rows, (reason, point), (steps, elapsed, mah, wh) = simulate(
            [int(value) for value in inputs])
        tolerance = oracle['tolerances']
        assert len(rows) == expected_rows or len(rows) == len(expected_rows)
        assert len(rows) == len(expected_rows), (len(rows), len(expected_rows))
        assert int(summary[1]) == steps and int(summary[2]) == elapsed
        assert summary[1] == str(case['steps']) or int(summary[1]) == case['steps']
        assert stop[1] == case['stop'] and stop[1] == reason
        near(emitted(summary[3]), mah, tolerance['charge_absolute_mAh'],
             tolerance['charge_relative'])
        near(emitted(summary[4]), wh, tolerance['energy_absolute_Wh'],
             tolerance['energy_relative'])
        near(emitted(summary[3]), D(case['delivered_mAh']), '0', '0')
        near(emitted(summary[4]), D(case['delivered_Wh']), '0', '0')
        near(emitted(stop[2]), point[0], tolerance['voltage_absolute_V'],
             tolerance['voltage_relative'])
        near(emitted(stop[3]), point[1], tolerance['voltage_absolute_V'],
             tolerance['voltage_relative'])
        near(emitted(stop[4]), point[2], tolerance['current_absolute_A'],
             tolerance['current_relative'])
        for index, (row, expected) in enumerate(zip(rows, expected_rows)):
            assert int(row[0]) == index
            elapsed_e, ocv, terminal, current, mah_e, wh_e = expected
            assert int(row[1]) == elapsed_e
            near(emitted(row[2]), ocv, tolerance['voltage_absolute_V'],
                 tolerance['voltage_relative'])
            near(emitted(row[3]), terminal, tolerance['voltage_absolute_V'],
                 tolerance['voltage_relative'])
            near(emitted(row[4]), current, tolerance['current_absolute_A'],
                 tolerance['current_relative'])
            near(emitted(row[5]), mah_e, tolerance['charge_absolute_mAh'],
                 tolerance['charge_relative'])
            near(emitted(row[6]), wh_e, tolerance['energy_absolute_Wh'],
                 tolerance['energy_relative'])
