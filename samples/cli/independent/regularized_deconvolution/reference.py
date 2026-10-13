"""Independent direct complex Fourier inversion, never the program's CG solver."""
import cmath
import hashlib
import json
import math
from pathlib import Path
ROOT = Path(__file__).resolve().parent

def oracle(y, kernel, regularization):
    n = len(y)
    spectrum = []
    for frequency in range(n):
        angle = 2 * math.pi * frequency / n
        transfer = sum(weight * cmath.exp(1j * angle * (len(kernel)//2-tap)) for tap, weight in enumerate(kernel))
        measured = sum(value * cmath.exp(-1j * angle * index) for index, value in enumerate(y))
        spectrum.append(transfer.conjugate() * measured / (abs(transfer)**2 + regularization*(2-2*math.cos(angle))))
    x = [sum(value * cmath.exp(2j*math.pi*frequency*index/n) for frequency,value in enumerate(spectrum)).real/n for index in range(n)]
    hx = [sum(weight*x[(index+len(kernel)//2-tap)%n] for tap,weight in enumerate(kernel)) for index in range(n)]
    residual = sum((a-b)**2 for a,b in zip(hx,y))
    roughness = sum((x[(i+1)%n]-x[i])**2 for i in range(n))
    return dict(reconstruction=x,reconvolution=hx,residual_norm=math.sqrt(residual),roughness=math.sqrt(roughness),objective=residual+regularization*roughness)

def fixtures():
    cases=[]
    def add(name,y,k,l):
        expected=oracle(y,k,l)
        cases.append(dict(name=name,y=y,kernel=k,regularization=l,expected=expected,input=' '.join(map(str,[len(y),len(k),l,*k,*y]))+'\n'))
    add('constant',[7]*8,[0.1,0.3,0.6],0.0001)
    # Analytic impulse-kernel response of I + lambda D^T D: cyclic Green function.
    for l in (0.0001,100):
        y=[1]+[0]*7
        add('impulse_'+str(l),y,[1],l)
        q=2*l/(1+2*l+math.sqrt(1+4*l))
        exact=[(q**i+q**(8-i))/((1-q**8)*math.sqrt(1+4*l)) for i in range(8)]
        assert max(abs(a-b) for a,b in zip(exact,cases[-1]['expected']['reconstruction']))<1e-12
    for n in (8,9,17,32):
        for l in (0.0001,100):
            add(f'asymmetric_{n}_{l}',[(i*i+3*i)%19 for i in range(n)],[0.05,0.1,0.2,0.25,0.4],l)
    add('signed',[0,0,0,1,0,0,0,0],[0.2,0.6,0.2],0.0001)
    assert min(cases[-1]['expected']['reconstruction'])< -0.1
    add('zero',[0]*32,[0,1,0],100)
    assert max(abs(x-7) for x in cases[0]['expected']['reconstruction'])<1e-12
    return cases

if __name__=='__main__':
    data=ROOT/'oracles.json'
    data.write_text(json.dumps(fixtures(),indent=2)+'\n')
    (ROOT/'oracle-freeze.json').write_text(json.dumps({p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in (Path(__file__),data)},indent=2)+'\n')
