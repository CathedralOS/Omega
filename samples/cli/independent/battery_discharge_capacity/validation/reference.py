"""Independent Decimal oracle. Freeze before any Omega check or execution.

Recomputes the constant-resistance linear-OCV discharge at 70-digit Decimal
precision. It shares no code with the Omega application and does not call or
translate its arithmetic. The stop reason, per-step operating points and the
charge/energy accumulations are all derived independently here.
"""
import hashlib
import json
from decimal import Decimal as D, localcontext
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOLERANCES = {
    'voltage_absolute_V': '2e-9', 'voltage_relative': '2e-8',
    'current_absolute_A': '5e-12', 'current_relative': '3e-8',
    'charge_absolute_mAh': '1e-6', 'charge_relative': '3e-8',
    'energy_absolute_Wh': '1e-9', 'energy_relative': '3e-8',
    'steps_slack': 0,
}


def simulate(values):
    """Exact model: returns (rows, stop, summary) as Decimal quantities."""
    full_mv, empty_mv, capacity_mah, internal_mohm, load_mohm, cutoff_mv, step_s, budget = map(int, values)
    full, empty, capacity = D(full_mv) / 1000, D(empty_mv) / 1000, D(capacity_mah) / 1000
    internal, load, cutoff = D(internal_mohm) / 1000, D(load_mohm) / 1000, D(cutoff_mv) / 1000
    step = D(step_s)
    charge = D(0)
    energy = D(0)
    elapsed = 0
    rows = []
    stop = None
    point = None
    while True:
        ocv = full - (full - empty) * charge / capacity
        current = ocv / (load + internal)
        terminal = ocv - current * internal
        point = (ocv, terminal, current)
        if terminal <= cutoff:
            stop = 'cutoff'
            break
        charge += current * step / 3600
        energy += terminal * current * step / 3600
        elapsed += step_s
        rows.append((elapsed, ocv, terminal, current, charge * 1000, energy))
        if charge >= capacity:
            stop = 'depleted'
            break
        if len(rows) >= budget:
            stop = 'budget'
            break
    return rows, (stop, point), (len(rows), elapsed, charge * 1000, energy)


def emit(value):
    """The application's emission grid: nine significant decimal digits,
    truncated (no rounding), as `mantissa e exponent` encodes."""
    value = D(value)
    if value == 0:
        return D(0)
    exponent = 0
    while value >= 10:
        value /= 10
        exponent += 1
    while value < 1:
        value *= 10
        exponent -= 1
    mantissa = int(value * D(10) ** 8)
    return D(mantissa) * D(10) ** (exponent - 8)


CASES = {
    'nominal_discharge': '4200 2500 3000 50 1000 2500 60 10000',
    'light_load_depletion': '4200 2500 3000 50 100000 1000 60 10000',
    'budget_limited': '4200 2500 100000 50 1000 2500 60 10',
    'heavy_load': '4200 2500 3000 50 100 2500 60 10000',
    'small_capacity': '4200 2500 10 50 1000 2500 60 10000',
    'high_voltage': '15000 5000 50000 100 5000 4000 120 5000',
    'instant_cutoff': '4200 2500 3000 50 1000 4199 60 10000',
    'crlf_record': '4200 2500 3000 50 1000 2500 60 10000',
    'eof_record': '4200 2500 3000 50 1000 2500 60 10000',
    'invalid_empty': '',
    'invalid_missing': '4200 2500 3000 50 1000 2500 60',
    'invalid_extra': '4200 2500 3000 50 1000 2500 60 10000 5',
    'invalid_number': '4200 2500 x000 50 1000 2500 60 10000',
    'invalid_overflow': '4200 2500 3000000000000 50 1000 2500 60 10000',
    'invalid_range_sign': '4200 2500 -50 50 1000 2500 60 10000',
    'invalid_low_full': '1500 2500 3000 50 1000 2500 60 10000',
    'invalid_inverted': '4200 5000 3000 50 1000 2500 60 10000',
    'invalid_cutoff': '4200 2500 3000 50 1000 4200 60 10000',
    'invalid_zero_load': '4200 2500 3000 50 0 2500 60 10000',
    'invalid_budget': '4200 2500 3000 50 1000 2500 60 20000',
    'invalid_length': '4' * 170,
}

INVALID = {
    'invalid_empty': 'syntax', 'invalid_missing': 'syntax', 'invalid_extra': 'syntax',
    'invalid_number': 'syntax', 'invalid_overflow': 'syntax', 'invalid_length': 'syntax',
    'invalid_range_sign': 'range', 'invalid_low_full': 'range', 'invalid_inverted': 'range',
    'invalid_cutoff': 'range', 'invalid_zero_load': 'range', 'invalid_budget': 'range',
}


def main():
    destination = ROOT / 'validation' / 'oracle.json'
    if destination.exists():
        raise SystemExit('oracle.json is frozen; refusing to overwrite')
    with localcontext() as context:
        context.prec = 70
        cases = {}
        for name, record in CASES.items():
            fixture = ROOT / 'data' / (name + '.stdin')
            assert fixture.read_bytes() == (record + ('\n' if name not in ('eof_record', 'invalid_empty', 'invalid_length') else '')).encode() \
                or name in ('crlf_record', 'eof_record', 'invalid_empty', 'invalid_length'), name
            entry = {'stdin_sha256': hashlib.sha256(fixture.read_bytes()).hexdigest()}
            if name in INVALID:
                entry['exit'] = 64 if INVALID[name] == 'syntax' else 65
                entry['error'] = INVALID[name]
            else:
                values = record.split()
                rows, stop, summary = simulate(values)
                entry['exit'] = 0
                entry['rows'] = len(rows)
                entry['stop'] = stop[0]
                entry['steps'], entry['elapsed'], mah, wh = summary
                entry['delivered_mAh'] = str(emit(mah))
                entry['delivered_Wh'] = str(emit(wh))
            cases[name] = entry
        destination.write_text(json.dumps({'tolerances': TOLERANCES, 'cases': cases},
                                          indent=2, sort_keys=True) + '\n')
        print('froze', destination)


if __name__ == '__main__':
    main()
