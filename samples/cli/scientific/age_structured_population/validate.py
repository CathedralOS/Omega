"""Compare a retained executable with immutable fixtures; never update oracles."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
ROOT = Path(__file__).resolve().parent
FROZEN_MANIFEST = "e1fe875c7b1eecae75af5defd9ac5de3cb7b0dbb06ec9186e996153548e557ed"
FROZEN_REFERENCE = "15e2150932e79112f7b3193fa6792c838fb804efc8cd9b26b92ed0210f36b7c4"
REQUIRED_CASES = frozenset("births extinction migration fractional zero_steps terminal_age maximum_horizon invalid_survival negative_rate invalid_fertility invalid_horizon negative_count invalid_migration infeasible_emigration capacity malformed missing overlong empty trailing net_emigration final_births large_integer".split())

def require(condition, message):
    if not condition:
        raise SystemExit("validation integrity failure: " + message)

manifest_bytes = (ROOT/'fixtures/oracle.json').read_bytes()
require(hashlib.sha256(manifest_bytes).hexdigest() == FROZEN_MANIFEST, "frozen oracle manifest changed")
require(hashlib.sha256((ROOT/'reference.py').read_bytes()).hexdigest() == FROZEN_REFERENCE, "frozen reference changed")
manifest = json.loads(manifest_bytes)
require(manifest['tolerance'] == 0, "nonzero tolerance")
require(set(manifest['cases']) == REQUIRED_CASES, "required 23-case inventory changed")
require(len(sys.argv) in (2, 3) and (len(sys.argv) == 2 or sys.argv[2] == "--both"), "usage: validate.py EXECUTABLE [--both]")
binary = Path(sys.argv[1]).resolve()
comparison = sys.argv[2:] == ["--both"]
command = [str(binary), "run", "--both", str(ROOT/"main.omg")] if comparison else [str(binary)]
fixtures = []
for name, case in manifest['cases'].items():
    data = (ROOT/'fixtures'/f'{name}.input').read_bytes()
    expected = (ROOT/'fixtures'/f'{name}.stdout').read_bytes()
    require(hashlib.sha256(data).hexdigest() == case['input_sha256'], name + ' input changed')
    require(hashlib.sha256(expected).hexdigest() == case['stdout_sha256'], name + ' expected stdout changed')
    fixtures.append((name, case, data, expected))
results = []
for name, case, data, expected in fixtures:
    result = subprocess.run(command, input=data, capture_output=True, timeout=120)
    agrees = result.stderr == f"native exit {case['exit']}\ninterp exit: {case['exit']}\n".encode() if comparison else not result.stderr
    good = result.returncode == case['exit'] and result.stdout == expected and agrees
    results.append({'case':name, 'exit':result.returncode, 'expected_exit':case['exit'], 'exact_stdout':result.stdout == expected, 'stderr':result.stderr.decode(errors='replace'), 'pass':good})
    print(f'{name}: {"PASS" if good else "FAIL"} exit={result.returncode}')
    out = ROOT/('build/terminal-validation' if comparison else 'build/validation')
    out.mkdir(parents=True, exist_ok=True)
    (out/f'{name}.stdout').write_bytes(result.stdout)
    (out/f'{name}.stderr').write_bytes(result.stderr)
(out/'results.json').write_text(json.dumps(results, indent=2)+'\n')
raise SystemExit(0 if all(r['pass'] for r in results) else 1)
