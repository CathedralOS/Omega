#!/usr/bin/env python3
"""Compare real Omega/native output with frozen independent reference artifacts."""
import argparse
import hashlib
import json
import re
from pathlib import Path
import subprocess
import time
ROOT = Path(__file__).resolve().parent
FROZEN_SHA256 = '8725271f0cb3cc403e695898157d925baaebaebe74045178229bc3ac2b6d9f33'

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', type=Path)
    parser.add_argument('--omega', type=Path)
    parser.add_argument('--both', action='store_true')
    parser.add_argument('--named-only', action='store_true')
    parser.add_argument('--case', action='append', help='Select an exact frozen case name')
    parser.add_argument('--timeout-seconds', type=float, default=600)
    parser.add_argument('--output', type=Path, required=True)
    arguments = parser.parse_args()
    if bool(arguments.executable) == bool(arguments.omega):
        parser.error('Choose exactly one of --executable or --omega')
    raw = (ROOT / 'data/frozen.json').read_bytes()
    assert hashlib.sha256(raw).hexdigest() == FROZEN_SHA256, 'Frozen oracle modified'
    reuse_raw = (ROOT / 'data/frozen_reuse.json').read_bytes()
    assert hashlib.sha256(reuse_raw).hexdigest() == '0bd8d5341a852594f1352b7fc1ce17f65eaf04b894df4a8d516c47b47c5fee44', 'Frozen reuse oracle modified'
    cases = json.loads(raw)['cases'] + json.loads(reuse_raw)['cases']
    if arguments.named_only:
        cases = [case for case in cases if not case['name'].startswith('tiny_')]
    if arguments.case:
        cases = [case for case in cases if case['name'] in arguments.case]
        assert {case['name'] for case in cases} == set(arguments.case), 'Unknown frozen case'
    records = []
    arguments.output.mkdir(parents=True, exist_ok=True)
    for case in cases:
        if arguments.executable:
            command = [str(arguments.executable.resolve())]
        else:
            command = [str(arguments.omega.resolve()), 'run']
            if arguments.both:
                command += ['--both']
            command += [str(ROOT / 'main.omg')]
        started = time.monotonic()
        observed = subprocess.run(command, input=case['input'].encode(), capture_output=True, timeout=arguments.timeout_seconds)
        valid = observed.returncode == case['exit'] and observed.stdout == case['output'].encode()
        comparison = None
        if arguments.both:
            diagnostic = observed.stderr.decode(errors='replace')
            match = re.search(r'^interp exit: (-?\d+)$', diagnostic, re.MULTILINE)
            comparison = 'declined' if 'DECLINED' in diagnostic else 'exit_agreement' if match and int(match.group(1)) == case['exit'] and 'DIVERGENCE' not in diagnostic else 'unclassified'
            # A decline is recorded honestly and cannot establish Terminal acceptance.
            valid = valid and comparison == 'exit_agreement'
        if not case['name'].startswith('tiny_') or not valid:
            (arguments.output / (case['name'] + '.stdout')).write_bytes(observed.stdout)
            (arguments.output / (case['name'] + '.stderr')).write_bytes(observed.stderr)
        records.append(dict(name=case['name'], command=command, exit=observed.returncode,
                            expected_exit=case['exit'], valid=valid, comparison=comparison,
                            stdout_sha256=hashlib.sha256(observed.stdout).hexdigest(),
                            stderr=observed.stderr.decode(errors='replace'), seconds=time.monotonic()-started))
        if not case['name'].startswith('tiny_'):
            print(f'{case["name"]}: {"PASS" if valid else "FAIL"} ({comparison})', flush=True)
        if not valid:
            print(f'FAIL {case["name"]}: exit {observed.returncode}, comparison {comparison}')
            break
    report = dict(frozen_sha256=FROZEN_SHA256, reuse_frozen_sha256=hashlib.sha256(reuse_raw).hexdigest(), target='linux_x86_64',
                  executable_sha256=hashlib.sha256((arguments.executable or arguments.omega).read_bytes()).hexdigest(),
                  selected=len(cases), tested=len(records), passed=sum(r['valid'] for r in records), records=records)
    (arguments.output / 'results.json').write_text(json.dumps(report, indent=2)+'\n')
    print(f'{report["passed"]}/{report["selected"]} cases passed; report {arguments.output / "results.json"}')
    return 0 if len(records) == len(cases) and all(r['valid'] for r in records) else 1

if __name__ == '__main__':
    raise SystemExit(main())
