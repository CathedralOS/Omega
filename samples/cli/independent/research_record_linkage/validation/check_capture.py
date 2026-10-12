"""Check actual admitted Terminal buffers against immutable sample oracles."""
from pathlib import Path
import argparse, hashlib, json

parser = argparse.ArgumentParser()
parser.add_argument('capture', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
for manifest in ('data/oracle.sha256', 'data/greedy_trap.sha256'):
    for line in (root/manifest).read_text().splitlines():
        digest, name = line.split('  ', 1)
        assert hashlib.sha256((root/name).read_bytes()).hexdigest() == digest, name
cases = json.loads((root/'data/frozen.json').read_text())['cases']
cases['greedy_trap'] = json.loads((root/'data/greedy_trap_frozen.json').read_text())
lines = (args.capture/'records.tsv').read_text().splitlines()
assert lines[0] == 'case\texit'
observed = {}
for line in lines[1:]:
    name, code = line.split('\t')
    assert name not in observed, name
    observed[name] = int(code)
inputs = {path.stem for path in (root/'data').glob('*.txt')}
assert set(observed) == inputs and len(inputs) == 39
for name, code in observed.items():
    invalid = name.startswith('invalid_')
    expected = b'INVALID input\n' if invalid else (root/'data'/f'{name}.stdout').read_bytes()
    expected_exit = 64 if invalid else cases[name]['exit']
    assert (args.capture/f'{name}.stdout').read_bytes() == expected, (name, 'stdout')
    assert not (args.capture/f'{name}.stderr').read_bytes(), (name, 'stderr')
    assert code == expected_exit, (name, 'exit', code, expected_exit)
print('39 captured Terminal stdout/stderr/exits match unchanged zero-tolerance oracles')
