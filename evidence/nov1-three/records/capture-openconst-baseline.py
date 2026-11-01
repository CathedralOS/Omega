import json,subprocess,sys
from pathlib import Path
root=Path('build/swarm/nov1');state=root/'cleanup-baseline.json'
for name in ['canonical_local','canonical_local_invalid','package_scalar']:
    rev=json.loads(state.read_text())['revision']
    r=subprocess.run(['python3','tools/qualification.py','run','--state',str(state),'--revision',str(rev),'--check',name],capture_output=True,text=True)
    (root/(name+'-capture-summary.json')).write_text(r.stdout)
    (root/(name+'-capture-controller.stderr')).write_text(r.stderr)
    print(name,'capture command exit',r.returncode,flush=True)
    if r.stdout:
        try:
            s=json.loads(r.stdout);print(json.dumps(s.get('latest_run',{})),flush=True)
        except ValueError:print(r.stdout[-1500:],flush=True)
    if r.stderr:print(r.stderr,end='',file=sys.stderr)
