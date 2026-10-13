"""Independent 90-digit exhaustive hidden-path oracle, frozen before execution."""
from decimal import Decimal, localcontext
from itertools import product
from pathlib import Path
import hashlib, json
ROOT=Path(__file__).resolve().parent

def infer(probabilities, observations):
    with localcontext() as context:
        context.prec=90
        p=[Decimal(value)/1000000 for value in probabilities]
        total=Decimal(0); posterior=[[Decimal(0),Decimal(0)] for _ in observations]
        best=Decimal(-1); path=None
        for hidden in product((0,1), repeat=len(observations)):
            weight=p[hidden[0]]*p[6+2*hidden[0]+observations[0]]
            for index in range(1,len(hidden)):
                weight*=p[2+2*hidden[index-1]+hidden[index]]*p[6+2*hidden[index]+observations[index]]
            total+=weight
            for index,state in enumerate(hidden): posterior[index][state]+=weight
            if weight>best: best=weight; path=hidden
        if total==0: return {'exit':65,'status':'IMPOSSIBLE'}
        return {'exit':0,'status':'OK','log_likelihood':str(total.ln()),
                'posterior':[[str(value/total) for value in row] for row in posterior],
                'path':list(path),'enumerated_paths':2**len(observations)}

def freeze():
    cases={}
    def add(name,p,o):
        raw=(''.join(f'{v:07d}\n' for v in p)+''.join(map(str,o))+'\n').encode()
        (ROOT/'data'/f'{name}.stdin').write_bytes(raw)
        cases[name]={'input_sha256':hashlib.sha256(raw).hexdigest(),**infer(p,o)}
    standard=[600000,400000,700000,300000,400000,600000,500000,500000,100000,900000]
    add('standard',standard,[0,1,0,1,1])
    add('standard_swapped',[standard[1],standard[0],standard[5],standard[4],standard[3],standard[2],standard[8],standard[9],standard[6],standard[7]],[0,1,0,1,1])
    add('ambiguous',[500000]*10,[0,1]*8)
    add('deterministic',[1000000,0,0,1000000,1000000,0,1000000,0,0,1000000],[0,1]*8)
    add('zero_transition',[400000,600000,1000000,0,0,1000000,300000,700000,800000,200000],[1,0,1])
    add('impossible',[1000000,0,1000000,0,0,1000000,1000000,0,0,1000000],[1])
    add('tiny',[999999,1,999999,1,1,999999,1,999999,1,999999],[0]*16)
    add('single',standard,[1])
    add('cross_tie',[500000,500000,100000,900000,900000,100000,200000,800000,800000,200000],[0,0])
    good=(ROOT/'data/standard.stdin').read_bytes()
    invalid={'empty':b'', 'bad_distribution':b'0600001\n'+good[8:],
             'negative':b'-000001\n'+good[8:], 'out_of_range':b'1000001\n'+good[8:],
             'bad_symbol':good[:-6]+b'01211\n','no_observations':good[:80]+b'\n',
             'capacity':good[:80]+b'0'*17+b'\n','partial':good[:-1],
             'extra':good+b'0\n','long_line':b'0'*65+b'\n'}
    for name,raw in invalid.items():
        (ROOT/'data'/f'{name}.stdin').write_bytes(raw)
        cases[name]={'input_sha256':hashlib.sha256(raw).hexdigest(),'exit':64,'status':'INVALID'}
    (ROOT/'expected.json').write_text(json.dumps({'precision':90,'absolute_tolerance':1e-9,'cases':cases},indent=2)+'\n')
if __name__=='__main__': freeze()
