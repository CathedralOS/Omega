"""Complete the configured-path screen with quality and independent comparison."""
import hashlib,json,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]; FOLDER=Path(__file__).resolve().parent; BASE=ROOT/'build/copy-propagation-replay'; DATA=BASE/'completion-v2';DATA.mkdir();steps=[]
def step(name,args):
    with (DATA/(name+'.stdout')).open('w',encoding='utf-8') as out,(DATA/(name+'.stderr')).open('w',encoding='utf-8') as err:
        result=subprocess.run([sys.executable,*args],cwd=ROOT,stdout=out,stderr=err)
    steps.append({'step':name,'exit':result.returncode});(DATA/'steps.json').write_text(json.dumps(steps,indent=2)+'\n');assert result.returncode==0,(name,result.returncode)
    print(name,'passed',flush=True)
previous=json.loads((BASE/'comparison/results.json').read_text());assert previous['complete']
assert all(r['native']['exit']==0 for k in ('seed','single','profiles') for r in previous[k])
print('Retain completed comparison observed at '+previous['controller_revision']+'; only reject metadata lookup/quality paths changed.',flush=True)
step('quality',[str(FOLDER/'qualify.py')])
step('confirmation',[str(FOLDER/'compare.py'),'confirmation'])
outputs={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for tree in (DATA,BASE/'comparison',BASE/'confirmation',BASE/'quality-v2',BASE/'quality-witnesses-v2') for p in tree.rglob('*') if p.is_file() and p.suffix in ('.json','.log','.txt','.stdout','.stderr','.omg')};assert outputs
print(json.dumps({'captured_outputs':outputs},sort_keys=True),flush=True)
