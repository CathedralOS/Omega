"""Independent pre-execution exact parser boundaries; all earlier freezes retained."""
import hashlib,json,math
from pathlib import Path
from scipy.integrate import solve_ivp
ROOT=Path(__file__).resolve().parents[1]
base='1 1 0 0 1 0.001 1'
texts={'fraction30':'1.'+'0'*30+' 1 0 0 1 0.001 1','fraction31':'1.'+'0'*31+' 1 0 0 1 0.001 1','read511':base+' '*(511-len(base)),'read512':base+' '*(512-len(base)),'whitespace_overlength':' '*513}
valid={'fraction30','read511'}
def derivative(t,z):
    factor=-1/math.hypot(z[0],z[1])**3
    return [z[2],z[3],factor*z[0],factor*z[1]]
solution=solve_ivp(derivative,[0,.001],[1,0,0,1],method='DOP853',rtol=2.3e-14,atol=2e-15,t_eval=[0,.001])
assert solution.success
cases={};paths=[Path(__file__).resolve()]
for name,text in texts.items():
    path=ROOT/'data'/f'{name}.stdin';path.write_text(text);paths.append(path)
    cases[name]={'exit':0,'status':'complete','input':[1,1,0,0,1,.001,1],'ode':[list(map(float,z)) for z in solution.y.T]} if name in valid else {'exit':64,'status':'invalid'}
p=ROOT/'validation/parser_oracle.json';p.write_text(json.dumps({'cases':cases,'method':'exact token/byte counts; DOP853 one-step reference for valid boundaries'},indent=2)+'\n');paths.append(p)
(ROOT/'validation/parser_frozen.sha256').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(ROOT))+'\n' for p in paths))
print('Frozen exact 30/31 fractional digit and 511/512 byte boundaries')
