"""Validate the frozen integrated producer, then measure the fixed-base comparison."""
import json,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
FOLDER=Path(__file__).resolve().parent
BASE=ROOT/'build/native-queue-landing'
steps=[]
for name in ('qualify.py','controls.py','partial_target.py','compare.py'):
    with (BASE/('validate-'+name+'.stdout')).open('w',encoding='utf-8') as out,(BASE/('validate-'+name+'.stderr')).open('w',encoding='utf-8') as err:
        result=subprocess.run([sys.executable,str(FOLDER/name)],cwd=ROOT,stdout=out,stderr=err)
    steps.append({'step':name,'exit':result.returncode})
    (BASE/'validation.json').write_text(json.dumps(steps,indent=2)+'\n',encoding='utf-8')
    assert result.returncode==0,(name,result.returncode)
    print(name,'passed',flush=True)
assert all(json.loads((BASE/name/'results.json').read_text(encoding='utf-8'))['complete'] for name in ('qualification','lookup-controls','partial-target-control','comparison'))
