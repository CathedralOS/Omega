"""Validate the real Omega-produced executable against frozen independent oracles."""
import argparse, hashlib, json, subprocess, time
from pathlib import Path
parser = argparse.ArgumentParser()
parser.add_argument("executable", type=Path)
parser.add_argument("--report", type=Path, default=Path("build/native-validation.json"))
arguments = parser.parse_args()
root = Path(__file__).resolve().parents[1]
manifest = json.loads((root/'validation/oracles.json').read_text())
assert manifest['tolerance'] == 0
assert hashlib.sha256((root/'validation/reference.py').read_bytes()).hexdigest() == manifest['reference_sha256']
executable = arguments.executable.resolve()
records = []
started = time.monotonic()
for case in manifest['cases']:
    payload = ' '.join(map(str, case['values']))+'\n'
    result = subprocess.run([str(executable)], input=payload, text=True, capture_output=True, timeout=20)
    assert (result.returncode, result.stdout, result.stderr) == (case['exit'], case['stdout'], ''), (case['name'], result.returncode, result.stdout, result.stderr)
    if result.returncode == 0:
        shipments = [int(value) for row in result.stdout.splitlines()[1:4] for value in row.split()]
        values = case['values']
        assert len(shipments) == 9 and all(quantity >= 0 for quantity in shipments)
        assert [sum(shipments[row*3:row*3+3]) for row in range(3)] == values[:3]
        assert [sum(shipments[column::3]) for column in range(3)] == values[3:6]
        assert sum(quantity*cost for quantity, cost in zip(shipments, values[6:])) == int(result.stdout.splitlines()[4].split()[1])
    records.append(dict(name=case['name'], exit=result.returncode, stdout=result.stdout))
for name, payload in manifest['invalid'].items():
    result = subprocess.run([str(executable)], input=payload, text=True, capture_output=True, timeout=20)
    assert (result.returncode, result.stdout, result.stderr) == (2, manifest['invalid_stdout'], ''), (name, result)
    records.append(dict(name='invalid_'+name, exit=result.returncode, stdout=result.stdout))
# EOF without newline and CRLF/tab-separated input exercise parser finalization.
for name, payload in [('eof', ' '.join(map(str, manifest['cases'][0]['values']))),
                      ('crlf_tabs', '\t'.join(map(str, manifest['cases'][0]['values']))+'\r\n')]:
    result = subprocess.run([str(executable)], input=payload, text=True, capture_output=True, timeout=20)
    assert (result.returncode, result.stdout, result.stderr) == (0, manifest['cases'][0]['stdout'], ''), name
    records.append(dict(name=name, exit=0, stdout=result.stdout))
report = dict(executable=str(executable), executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
              oracle_sha256=hashlib.sha256((root/'validation/oracles.json').read_bytes()).hexdigest(),
              elapsed_seconds=time.monotonic()-started, records=records, count=len(records), tolerance=0)
arguments.report.parent.mkdir(parents=True, exist_ok=True)
arguments.report.write_text(json.dumps(report, indent=2)+'\n')
print(f"PASS: {len(records)} native cases; exact output, exits, conservation, objective and exhaustive tiny optimality")
