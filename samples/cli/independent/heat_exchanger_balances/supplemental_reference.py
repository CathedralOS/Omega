"""Freeze independent-review adversaries without modifying the original oracle."""
from pathlib import Path
import hashlib
import json
from reference import reference, milli

destination = Path(__file__).resolve().parent / 'data'
valid = {
    'rounding_tie': b'1 1 1 9 1 17 100 0 3\n',
    'no_trailing_newline': b'1 1 2 5 2 5 100 20 5',
    'leading_zeros': b'01 01 02 05 02 05 0100 020 05\n',
    'byte_255': b'1 1 2 5 2 5 100 20 5' + b' ' * 235,
}
invalid = {
    'byte_256_valid_prefix': valid['byte_255'] + b' ',
    'seventeenth_token': b'1 8 2 5 3 4 180 15 0 1 2 3 4 5 6 7 0\n',
}

def freeze():
    entries = []
    for name, data in valid.items():
        hot, cold, heat, total = reference(list(map(int, data.split())))
        rows = ['heat_exchanger_balances midpoint counterflow; temperature=mdegC heat=mW']
        for position in range(len(hot)):
            rows.append(f'node hot cold: {milli(hot[position])} {milli(cold[position])}')
            if position < len(heat):
                rows.append(f'stage heat: {milli(heat[position])}')
        rows += [f'total heat: {milli(total)}', 'PASS enthalpy and cold boundary tolerance=1e-7']
        (destination / (name + '.txt')).write_bytes(data)
        (destination / (name + '.stdout')).write_text('\n'.join(rows) + '\n')
        entries.append(dict(name=name, exit=0, input_sha256=hashlib.sha256(data).hexdigest(),
                            hot=list(map(str, hot)), cold=list(map(str, cold)),
                            heat=list(map(str, heat)), total=str(total)))
    for name, data in invalid.items():
        (destination / (name + '.txt')).write_bytes(data)
        (destination / (name + '.stdout')).write_text('INVALID bounded counterflow input\n')
        entries.append(dict(name=name, exit=64, input_sha256=hashlib.sha256(data).hexdigest()))
    (destination / 'supplemental_oracle.json').write_text(json.dumps(dict(
        freeze='before any Omega execution of these additional inputs',
        original_oracle_sha256='fba8b0b7e3bde8a7795425d6691f1cdeeb64d29174fcd32fe22ea9d4c241a32c',
        display_tie_tolerance=1e-7, cases=entries), indent=2) + '\n')

if __name__ == '__main__':
    freeze()
