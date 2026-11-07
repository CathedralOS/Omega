from pathlib import Path
import json, hashlib
from fractions import Fraction
import argparse
parser=argparse.ArgumentParser()
parser.add_argument('--output', type=Path, default=Path(__file__).resolve().parent/'build/reference')
p=parser.parse_args().output
(p/'data').mkdir(parents=True,exist_ok=True)
signal=[0,1,-2,3,-1,2,0,-3]
def delayed(delay): return [signal[k-delay] if 0<=k-delay<8 else 0 for k in range(8)]
def record(x,y,delay,enabled=1,timestamp=0): return [enabled,timestamp,x,y]+[v+4 for v in delayed(delay)]
cases={
'east':[record(0,1,-1),record(1,1,0),record(2,1,1)],
'west':[record(0,1,1),record(1,1,0),record(2,1,-1)],
'north':[record(1,0,-1),record(1,1,0),record(1,2,1)],
'dropout':[record(0,1,-1),record(1,1,0,0,2),record(2,1,1)],
'colocated':[record(1,1,0),record(1,1,0),record(1,1,0)],
'silence':[[1,0,0,0]+[4]*8,[1,0,1,1]+[4]*8,[1,0,2,2]+[4]*8],
}
cases['broadside']=[record(0,1,0),record(1,1,0),record(2,1,0)]
cases['positive_limit']=[[1,0,x,1]+[8]*8 for x in range(3)]
cases['negative_limit']=[[1,0,x,1]+[0]*8 for x in range(3)]
noisy=[r[:] for r in cases['east']]; noisy[1][6]+=1
cases['noise']=noisy
bad={}
for name,field,value in [('timing',1,1),('geometry',2,3),('flag',0,2)]:
    rows=[r[:] for r in cases['east']]; rows[0][field]=value; bad[name]=(rows,2 if name=='timing' else 1)
bad['all_dropout']=([[0,0,1,1]+[4]*8 for _ in range(3)],3)
manifest={}
for name,rows in cases.items():
    (p/'data'/f'{name}.stdin').write_text('\n'.join(''.join(map(str,r)) for r in rows)+'\n')
    active=[r for r in rows if r[0]]; n=len(active)
    directions=[(1,0),(-1,0),(0,1),(0,-1)]
    wave=[[sum(Fraction(r[4+t+(r[2]-1)*dx+(r[3]-1)*dy]-4,n) for r in active) for t in range(2,6)] for dx,dy in directions]
    energy=[sum(v*v for v in values) for values in wave]
    winners=[i for i,e in enumerate(energy) if e==max(energy)]
    lines=[f'ACTIVE {n}']+[f'DIRECTION {d} WAVE '+ ' '.join(f'{int(v*n)}/{n}' for v in values)+f' ENERGY {int(energy[d]*n*n)}/{n*n}' for d,values in enumerate(wave)]+['WINNERS '+' '.join(map(str,winners))]
    (p/'data'/f'{name}.stdout').write_text('\n'.join(lines)+'\n')
    manifest[name]={'exit':0,'waveform':[[str(v) for v in row] for row in wave],'energy':list(map(str,energy)),'winners':winners,'absolute_tolerance':'0','relative_tolerance':'0'}
for name,(rows,status) in bad.items():
    (p/'data'/f'{name}.stdin').write_text('\n'.join(''.join(map(str,r)) for r in rows)+'\n')
    (p/'data'/f'{name}.stdout').write_text(f'ERROR {status}\n'); manifest[name]={'exit':64}
for name,text in [('truncated','101144'),('trailing',(p/'data/east.stdin').read_text()+'4'),('bad_digit',(p/'data/east.stdin').read_text().replace('1','9',1)),('empty','')]:
    (p/'data'/f'{name}.stdin').write_text(text); (p/'data'/f'{name}.stdout').write_text('ERROR 10\n'); manifest[name]={'exit':64}
(p/'data/oracle.json').write_text(json.dumps(manifest,indent=2)+'\n')
(p/'data/FROZEN.sha256').write_text(''.join(hashlib.sha256(f.read_bytes()).hexdigest()+'  '+f.name+'\n' for f in sorted((p/'data').iterdir()) if f.name!='FROZEN.sha256'))
print((p/'data/east.stdout').read_text()); print((p/'data/dropout.stdout').read_text())
