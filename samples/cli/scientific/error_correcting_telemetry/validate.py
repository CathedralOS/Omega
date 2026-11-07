#!/usr/bin/env python3
"""Serial public CLI validation against frozen artifacts; never accepts packages."""
import argparse, hashlib, json, os, subprocess
from pathlib import Path
ROOT = Path(__file__).resolve().parent

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--omega', required=True)
    arguments = parser.parse_args()
    compiler = str(Path(arguments.omega).resolve())
    output = ROOT/'build'/'validation'
    output.mkdir(parents=True, exist_ok=True)
    oracle = json.loads((ROOT/'data'/'oracle.json').read_text())
    for name, digest in oracle['sha256'].items():
        assert hashlib.sha256((ROOT/'data'/name).read_bytes()).hexdigest() == digest, name
    results = []
    def run(label, command, payload=b'', expected=None, status=0, fragment=None):
        observed = subprocess.run(command, input=payload, capture_output=True)
        (output/(label+'.stdout')).write_bytes(observed.stdout)
        (output/(label+'.stderr')).write_bytes(observed.stderr)
        passed = observed.returncode == status
        if expected is not None:
            passed = passed and observed.stdout == expected
        if fragment is not None:
            passed = passed and fragment in observed.stderr.decode(errors='replace')
        row = {'label':label,'command':command,'exit':observed.returncode,'passed':passed}
        if '--both' in command:
            row['terminal'] = observed.stderr.decode(errors='replace')
            passed = passed and b'interp exit: 0' in observed.stderr and b'DECLINED' not in observed.stderr
            row['passed'] = passed
        results.append(row)
        (output/'results.json').write_text(json.dumps(results,indent=2)+'\n')
        print(json.dumps(row), flush=True)
        return passed
    entry = str(ROOT/'main.omg')
    if not run('check', [compiler,'--check',entry]):
        raise SystemExit(1)
    for name, status in [('exhaustive',0),('empty',0),('truncated',0),('oversized',3),('limit',3)]:
        run('native-'+name,[compiler,'run',entry],(ROOT/'data'/(name+'.stdin')).read_bytes(),
            (ROOT/'data'/(name+'.stdout')).read_bytes(),status)
    frozen_rows = dict(zip((ROOT/'data'/'exhaustive.stdin').read_bytes().splitlines(keepends=True),
                           (ROOT/'data'/'exhaustive.stdout').read_bytes().splitlines(keepends=True)))
    selected = [b'E23\n',b'D2031\n',b'D3031\n',b'D2131\n',b'D2041\n',b'D2032\n',
                b'D_031\n',b'D_0_1\n',b'D___1\n',b'D____\n',b'D0011\n',b'D0129\n']
    # The public CLI compares exits only; stdout here is native stdout.
    run('terminal-exit-variants',[compiler,'run','--both',entry],b''.join(selected),
        b''.join(frozen_rows[command] for command in selected))
    run('invalid-bounds',[compiler,'--check',str(ROOT/'controls'/'out_of_bounds'/'main.omg')],
        status=1,fragment=(ROOT/'controls'/'out_of_bounds'/'expected.txt').read_text().strip())
    raise SystemExit(0 if all(row['passed'] for row in results) else 1)

if __name__ == '__main__':
    main()
