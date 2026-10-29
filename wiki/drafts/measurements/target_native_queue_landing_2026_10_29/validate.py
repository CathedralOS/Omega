"""Validate the frozen integrated producer, then measure the fixed-base comparison."""
import json,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
FOLDER=Path(__file__).resolve().parent
BASE=ROOT/'build/native-queue-landing'
CAPTURE=BASE/'capture-2'; CAPTURE.mkdir()
steps=[]
for name in ('qualify.py','controls.py','partial_target.py','scalar_control.py','compare.py'):
    with (CAPTURE/('validate-'+name+'.stdout')).open('w',encoding='utf-8') as out,(CAPTURE/('validate-'+name+'.stderr')).open('w',encoding='utf-8') as err:
        command=[sys.executable,str(FOLDER/name)]+(['candidate'] if name=='scalar_control.py' else [])
        result=subprocess.run(command,cwd=ROOT,stdout=out,stderr=err)
    steps.append({'step':name,'exit':result.returncode})
    (CAPTURE/'validation.json').write_text(json.dumps(steps,indent=2)+'\n',encoding='utf-8')
    assert result.returncode==0,(name,result.returncode)
    print(name,'passed',flush=True)
assert all(json.loads((CAPTURE/name/'results.json').read_text(encoding='utf-8'))['complete'] for name in ('qualification','lookup-controls','partial-target-control','comparison'))
