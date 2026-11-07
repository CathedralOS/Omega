"""Observe actual native results and compare every reported numerical column."""
import argparse,hashlib,json,subprocess
from pathlib import Path
from check_output import check,ROOT
p=argparse.ArgumentParser(); p.add_argument('--omega'); p.add_argument('--executable'); a=p.parse_args()
destination=ROOT/'build/validation/native'; destination.mkdir(parents=True,exist_ok=True)
records=[]
fixtures=sorted((ROOT/'data').glob('*.stdin'))
oracle=json.loads((ROOT/'validation/oracle.json').read_text())
assert {fixture.stem for fixture in fixtures}==set(oracle), 'incomplete native fixture set'
for fixture in fixtures:
    command=[a.executable] if a.executable else [a.omega,'run','--keep',str(ROOT/'main.omg')]
    result=subprocess.run(command,input=fixture.read_bytes(),capture_output=True,timeout=900)
    (destination/(fixture.stem+'.stdout')).write_bytes(result.stdout)
    (destination/(fixture.stem+'.stderr')).write_bytes(result.stderr)
    records.append({'case':fixture.stem,'command':command,'exit':result.returncode,'sha256':hashlib.sha256(result.stdout).hexdigest()})
    (destination/'records.json').write_text(json.dumps(records,indent=2)+'\n')
    check(fixture.stem,result.stdout.decode(),result.returncode)
    print('native verified',fixture.stem,flush=True)
