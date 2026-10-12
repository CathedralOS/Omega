"""Independent qualification: SciPy Radau/BDF plus exact linear-chain formulas.

This script never implements the application's backward-Euler controller.
Reference files are frozen before observing any application numeric result.
"""
import hashlib
import json
import math
from pathlib import Path

import numpy as np
import scipy
from scipy.integrate import solve_ivp

ROOT = Path(__file__).resolve().parent

def rhs(tokens):
    rates = np.array([tokens[i] / tokens[i + 1] for i in range(0, 6, 2)])
    reactants = np.array([tokens[i:i+3] for i in (6,12,18)])
    products = np.array([tokens[i+3:i+6] for i in (6,12,18)])
    stoichiometry = (products - reactants).T
    def function(time, concentration):
        return stoichiometry @ (rates * np.prod(concentration[None, :] ** reactants, axis=1))
    return function

def input_tokens(model, end, tolerance, trials=100000, iterations=16):
    if model == 'robertson':
        values = [4,100,10000,1,30000000,1, 1,0,0,0,1,0, 0,1,1,1,0,1, 0,2,0,0,1,1]
    elif model == 'equal_chain':
        values = [2,1,2,1,0,1, 1,0,0,0,1,0, 0,1,0,0,0,1, 0,0,0,0,0,0]
    else:
        values = [1,1,100,1,0,1, 1,0,0,0,1,0, 0,1,0,0,0,1, 0,0,0,0,0,0]
    values += [1,1,0,1,0,1, *end, 1,1, 1,10**12, 1,tolerance, trials,iterations]
    assert len(values) == 40
    return values

def generate():
    cases = []
    for model, end in [('robertson',(2,5)),('robertson',(4,1)),('robertson',(40,1)),('chain',(1,1)),('equal_chain',(2,1))]:
        for denominator in (1000,100000,10000000):
            name = f'{model}_{end[0]}_{end[1]}_r{denominator}'
            tokens = input_tokens(model,end,denominator)
            stop = end[0]/end[1]
            references = {}
            for method in ('Radau','BDF'):
                result = solve_ivp(rhs(tokens),(0,stop),[1,0,0],method=method,rtol=1e-11,atol=1e-14,t_eval=[stop])
                assert result.success, result.message
                references[method] = result.y[:,-1].tolist()
                assert abs(sum(references[method])-1) < 1e-11
            assert np.max(abs(np.array(references['Radau'])-references['BDF'])) < 2e-9
            if model != 'robertson':
                k1,k2 = (2,2) if model == 'equal_chain' else (1,100)
                a = math.exp(-k1*stop)
                b = k1*stop*a if k1 == k2 else k1*(a-math.exp(-k2*stop))/(k2-k1)
                references['analytic'] = [a,b,1-a-b]
                assert np.max(abs(np.array(references['Radau'])-references['analytic'])) < 1e-10
            (ROOT/'data'/f'{name}.stdin').write_text(' '.join(map(str,tokens))+'\n')
            cases.append(dict(name=name,model=model,time=stop,rtol=1/denominator,expected_exit=0,reference=references))
    for name in ('stationary', 'one_correction_chain'):
        tokens = input_tokens('chain', (1,1), 100000, iterations=1)
        if name == 'stationary':
            tokens[0] = tokens[2] = 0
        references = {}
        for method in ('Radau','BDF'):
            result = solve_ivp(rhs(tokens),(0,1),[1,0,0],method=method,rtol=1e-11,atol=1e-14,t_eval=[1])
            assert result.success
            references[method] = result.y[:,-1].tolist()
        if name == 'stationary':
            references['analytic'] = [1,0,0]
        else:
            a = math.exp(-1); b = (a-math.exp(-100))/99
            references['analytic'] = [a,b,1-a-b]
        assert np.max(abs(np.array(references['Radau'])-references['analytic'])) < 1e-10
        (ROOT/'data'/f'{name}.stdin').write_text(' '.join(map(str,tokens))+'\n')
        cases.append(dict(name=name,model=name,time=1,rtol=1e-5,expected_exit=0,reference=references,refinement=False))
    base = input_tokens('robertson',(4,1),100000)
    bad = {'empty':[], 'missing':base[:-1], 'extra':base+[1]}
    for name,index,value in [('zero_rate_denominator',1,0),('zero_initial_denominator',25,0),('zero_atol',34,0),('zero_rtol',36,0),('zero_time',30,0),('zero_step',32,0),('bad_stoichiometry',9,2),('order_three',6,3),('invalid_iterations',39,33),('zero_budget',38,0)]:
        tokens = base.copy(); tokens[index] = value
        if name == 'order_three':
            # Sum remains three with product coefficients individually <=2.
            # Only the first reactant's coefficient exceeds the supported bound.
            tokens[9:12] = [1,2,0]
        bad[name] = tokens
    for name,tokens in bad.items():
        (ROOT/'data'/f'{name}.stdin').write_text(' '.join(map(str,tokens))+'\n')
        cases.append(dict(name=name,expected_exit=2))
    for name,text in [('negative','-1 '+ ' '.join(map(str,base[1:]))),('nan','NaN'),('infinity','inf'),('overflow','9999999999999999999999'),('bytes',' '*4097),('trailing_junk',' '.join(map(str,base))+' x')]:
        (ROOT/'data'/f'{name}.stdin').write_text(text+'\n');cases.append(dict(name=name,expected_exit=2))
    for name,trials,iterations in [('trial_exhaustion',1,16),('newton_exhaustion',100,1)]:
        tokens=input_tokens('robertson',(4,1),100000,trials,iterations)
        (ROOT/'data'/f'{name}.stdin').write_text(' '.join(map(str,tokens))+'\n');cases.append(dict(name=name,expected_exit=3))
    (ROOT/'data/oracles.json').write_text(json.dumps(dict(scipy=scipy.__version__,reference_rtol=1e-11,reference_atol=1e-14,cases=cases),indent=2)+'\n')
    files = [ROOT/'reference.py', *sorted((ROOT/'data').glob('*.stdin')), ROOT/'data/oracles.json']
    (ROOT/'data/FROZEN.sha256').write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(ROOT)}\n' for path in files))
    print(f'Frozen {len(cases)} independent fixtures')

if __name__ == '__main__':
    generate()
