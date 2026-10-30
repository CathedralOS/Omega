"""Fixed-source landing comparison, full quality and independent confirmation."""
import hashlib,json,statistics,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]; FOLDER=Path(__file__).resolve().parent; BASE=ROOT/'build/windows-custody-landing'
CAPTURE=BASE/'capture'; CAPTURE.mkdir(); steps=[]
def retain(): (CAPTURE/'experiment.json').write_text(json.dumps({'steps':steps},indent=2)+'\n',encoding='utf-8')
def step(name):
    with (CAPTURE/('step-'+name+'.stdout')).open('w',encoding='utf-8') as out,(CAPTURE/('step-'+name+'.stderr')).open('w',encoding='utf-8') as err:
        result=subprocess.run([sys.executable,str(FOLDER/name)],cwd=ROOT,stdout=out,stderr=err)
    steps.append({'step':name,'exit':result.returncode}); retain(); assert result.returncode==0,(name,result.returncode)
    print(name,'passed',flush=True)
step('compare.py')
data=json.loads((CAPTURE/'comparison/results.json').read_text(encoding='utf-8')); assert data['complete']
med={s:{f:statistics.median(r['wall_ms'] for r in data['single'] if r['subject']==s and r['flavor']==f) for f in data['producers']} for s in ('length','dispatch','stack32')}
reduction={s:1-v['candidate']/v['baseline'] for s,v in med.items()}
promising=all(reduction[s]>=0.15 for s in ('length','dispatch'))
(CAPTURE/'screen-decision.json').write_text(json.dumps({'medians':med,'reductions':reduction,'promising':promising,'protocol':'Preliminary 15 percent signal only; changed-source landing always runs full quality and independent confirmation.'},indent=2)+'\n',encoding='utf-8')
print('screen',reduction,'promising',promising,flush=True)
for name in ('qualify.py','controls.py','junction_controls.py','confirm.py'): step(name)
outputs={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in CAPTURE.rglob('*') if p.is_file() and p.suffix in ('.json','.log','.stdout','.stderr')}
assert outputs; print(json.dumps({'captured_outputs':outputs},sort_keys=True),flush=True)
