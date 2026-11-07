"""Sequential CLI checks against immutable, independently frozen exact oracles."""
from pathlib import Path
import argparse, hashlib, json, subprocess, sys, os
root=Path(__file__).resolve().parent
parser=argparse.ArgumentParser()
parser.add_argument('--omega',required=True)
parser.add_argument('--native',help='Optional already produced executable for all stdin cases')
parser.add_argument('--both',action='store_true',help='Replay each case through native and Terminal')
arguments=parser.parse_args()
subprocess.run([sys.executable,str(root/'reference.py')],check=True)
logs=root/('build/validation-both' if arguments.both else 'build/validation-native'); logs.mkdir(parents=True,exist_ok=True)
records=[]
for fixture in sorted((root/'data').glob('*.stdin')):
    invalid=fixture.name.startswith('invalid_')
    command=[arguments.native] if arguments.native else [arguments.omega,'run']+(['--both'] if arguments.both else [])+['--keep',str(root/'main.omg')]
    outcome=subprocess.run(command,input=fixture.read_bytes(),capture_output=True,timeout=120)
    (logs/f'{fixture.stem}.stdout').write_bytes(outcome.stdout)
    (logs/f'{fixture.stem}.stderr').write_bytes(outcome.stderr)
    expected=(root/'expected'/f'{fixture.stem}.stdout').read_bytes()
    expected_exit=2 if invalid else 0
    passed=outcome.returncode==expected_exit and outcome.stdout==expected
    if arguments.native: passed=passed and outcome.stderr==b''
    if arguments.both:
        # CLI explicitly reports comparison agreement on stderr; output alone is insufficient.
        passed=passed and f'interp exit: {expected_exit}'.encode() in outcome.stderr and b'DIVERGENCE' not in outcome.stderr and b'DECLINED' not in outcome.stderr
    records.append({'case':fixture.stem,'command':command,'exit':outcome.returncode,'expected_exit':expected_exit,'stdout_sha256':hashlib.sha256(outcome.stdout).hexdigest(),'expected_sha256':hashlib.sha256(expected).hexdigest(),'pass':passed})
    print(f'{fixture.stem}: {"PASS" if passed else "FAIL"} exit={outcome.returncode}',flush=True)
    if not passed:
        print(outcome.stderr.decode(errors='replace')[-5000:],flush=True)
        (logs/'results.json').write_text(json.dumps(records,indent=2)+'\n')
        sys.exit(1)
(logs/'results.json').write_text(json.dumps(records,indent=2)+'\n')
print(f'{len(records)} exact output/exit oracles passed.')
