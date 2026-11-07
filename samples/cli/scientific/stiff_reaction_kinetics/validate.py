"""Validate actual native or direct Terminal captures against frozen references."""
import argparse
import hashlib
import json
import math
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--executable', type=Path)
    parser.add_argument('--terminal-sponsor', type=Path)
    parser.add_argument('--captures', type=Path, help='reuse completed direct Terminal captures')
    parser.add_argument('--timeout', type=int, default=1200)
    args = parser.parse_args()
    assert bool(args.executable) != bool(args.terminal_sponsor), 'choose exactly one engine'
    for line in (ROOT/'data/FROZEN.sha256').read_text().splitlines():
        digest, path = line.split('  ',1)
        assert hashlib.sha256((ROOT/path).read_bytes()).hexdigest() == digest, path
    roster = json.loads((ROOT/'data/oracles.json').read_text())['cases']
    route = 'terminal' if args.terminal_sponsor else 'native'
    output = ROOT/'build'/'validation'
    output.mkdir(parents=True,exist_ok=True)
    assert not args.captures or args.terminal_sponsor, 'captures require the exact Terminal sponsor'
    captures = args.captures.resolve() if args.captures else Path(tempfile.mkdtemp(prefix=route+'-',dir=output))
    names = captures/'names.txt'
    names.write_text(''.join(case['name']+'\n' for case in roster))
    if args.terminal_sponsor and not args.captures:
        command = [str(args.terminal_sponsor.resolve()), str(ROOT), str(names), str(captures)]
        completed = subprocess.run(command,capture_output=True,timeout=args.timeout)
        (captures/'sponsor.stdout').write_bytes(completed.stdout)
        (captures/'sponsor.stderr').write_bytes(completed.stderr)
        assert completed.returncode == 0, completed.stderr.decode(errors='replace')[-4000:]
    records=[]
    groups={}
    for case in roster:
        name=case['name']
        if args.executable:
            started=time.monotonic()
            completed=subprocess.run([str(args.executable.resolve())],input=(ROOT/'data'/f'{name}.stdin').read_bytes(),capture_output=True,timeout=args.timeout)
            stdout,stderr,exit_code=completed.stdout,completed.stderr,completed.returncode
            (captures/f'{name}.stdout').write_bytes(stdout)
            (captures/f'{name}.stderr').write_bytes(stderr)
            (captures/f'{name}.exit').write_text(str(exit_code))
        else:
            stdout=(captures/f'{name}.stdout').read_bytes();stderr=(captures/f'{name}.stderr').read_bytes();exit_code=int((captures/f'{name}.exit').read_text())
        assert exit_code == case['expected_exit'], (name,exit_code,stdout,stderr)
        assert stderr == b'', (name,stderr)
        seconds=time.monotonic()-started if args.executable else float((captures/f'{name}.seconds').read_text())
        record=dict(name=name,exit=exit_code,seconds=seconds)
        if exit_code == 0:
            lines=stdout.decode().splitlines()
            assert len(lines)==3 and lines[0]=='stiff_reaction_kinetics units=1e-12', (name,stdout)
            observed_time,a,b,c,mass=map(lambda value:int(value)*1e-12,lines[1].split())
            accepted,rejected=map(int,lines[2].split())
            assert all(math.isfinite(x) and x>=0 for x in (a,b,c,mass))
            assert abs(observed_time-case['time'])<1e-9 and accepted>0
            assert abs(a+b+c-1)<1e-8 and mass<1e-8, (name,'mass',a+b+c,mass)
            reference=case['reference'].get('analytic',case['reference']['Radau'])
            error=max(abs(x-y) for x,y in zip((a,b,c),reference))
            bound=2*math.sqrt(case['rtol'])
            assert error<bound, (name,error,bound)
            record.update(error=error,mass_error=mass,accepted=accepted,rejected=rejected)
            if case.get('refinement',True):
                groups.setdefault((case['model'],case['time']),[]).append((case['rtol'],error))
        else:
            assert stdout == (b'INVALID reaction input\n' if exit_code==2 else b'EXHAUSTED integration convergence\n'), (name,stdout)
        records.append(record); print(route,name,'PASS',flush=True)
    for key,values in groups.items():
        ordered=sorted(values,reverse=True)
        assert ordered[2][1]<ordered[1][1]<ordered[0][1], (key,'tolerance refinement',ordered)
    binary=args.terminal_sponsor or args.executable
    report=dict(engine=route,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),records=records,captures=str(captures))
    (output/f'{route}-results.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f'{route}: {len(records)} cases and {len(groups)} tolerance-refinement groups verified')

if __name__=='__main__':
    main()
