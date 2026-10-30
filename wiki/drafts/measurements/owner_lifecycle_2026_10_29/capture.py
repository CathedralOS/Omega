"""One bounded attribution capture with source/format/runtime controls."""
import hashlib,json,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];FOLDER=Path(__file__).resolve().parent;BASE=ROOT/'build/owner-lifecycle';DATA=BASE/'capture';DATA.mkdir();steps=[]
for name in ('profile.py','qualify.py','controls.py','junction_controls.py'):
    with (DATA/('step-'+name+'.stdout')).open('w',encoding='utf-8') as out,(DATA/('step-'+name+'.stderr')).open('w',encoding='utf-8') as err:
        result=subprocess.run([sys.executable,str(FOLDER/name)],cwd=ROOT,stdout=out,stderr=err)
    steps.append({'step':name,'exit':result.returncode});(DATA/'steps.json').write_text(json.dumps(steps,indent=2)+'\n');assert result.returncode==0,(name,result.returncode)
    print(name,'passed',flush=True)
outputs={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for tree in (DATA,BASE/'diagnostic-profile') for p in tree.rglob('*') if p.is_file() and p.suffix in ('.json','.log','.stdout','.stderr')}
assert outputs;print(json.dumps({'captured_outputs':outputs},sort_keys=True),flush=True)
