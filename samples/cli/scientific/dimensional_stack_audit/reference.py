#!/usr/bin/env python3
"""Independent endpoint enumeration. Run --freeze BEFORE any Omega invocation."""
import hashlib, itertools, json, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parent
HEADER = 'dimensional_stack_audit: profile 1..6, temperature low, temperature high (one integer per line)\n'
INVALID = 'INVALID INPUT: expected profile 1..6 and ordered temperatures in [-100,100]\n'

def enumerate_case(profile, temperature):
    rows, bounds = profile['contributors'], profile['bounds']
    a_low,a_high,b_low,b_high,fit_low,fit_high = bounds
    # Independently evaluate each dimension at every joint endpoint: no
    # aggregate interval formula, no min/max coefficient algorithm.
    axes = [(-r[1],r[1]) for r in rows] + [tuple(temperature),(a_low,a_high),(b_low,b_high)]
    vertices = []
    for vertex in itertools.product(*axes):
        errors, (temp,a,b) = vertex[:3], vertex[3:]
        deviations = [errors[i]+r[2]*temp+r[3]*a+r[4]*b for i,r in enumerate(rows)]
        vertices.append((sum(r[0]+d for r,d in zip(rows,deviations)),deviations,vertex))
    low = min(v[0] for v in vertices)
    high = max(v[0] for v in vertices)
    # Tie policy is declared output behavior: lower lexicographic endpoint
    # witness for minimum; upper for maximum (including zero coefficients).
    lower = next(v for v in vertices if v[0] == low)
    upper = next(v for v in reversed(vertices) if v[0] == high)
    status = 'GUARANTEED' if fit_low <= low and high <= fit_high else 'IMPOSSIBLE' if high < fit_low or low > fit_high else 'CONDITIONAL'
    output = HEADER + f'fit_low_um\n{low}\nfit_high_um\n{high}\n{status}\n'
    offenders = []
    for i in range(3):
        for side, witness, violation, adverse in [('lower',lower,low < fit_low,lambda d:d < 0),('upper',upper,high > fit_high,lambda d:d > 0)]:
            deviation = witness[1][i]
            if violation and adverse(deviation):
                output += f'{side}_offender_index_zero_based\n{i}\ndeviation_um\n{deviation}\n'
                offenders.append([side,i,deviation])
    return dict(low=low,high=high,status=status,offenders=offenders,vertices=len(vertices)), output

def freeze():
    fixtures = json.loads((ROOT/'data/fixtures.json').read_text())['fixtures']
    cases=[]
    for i in range(1,7):
        cases.append((f'profile_{i}',f'{i}\n-10\n20\n',i,[-10,20]))
    cases += [('warm', '3\n0\n0\n',3,[0,0]),('cold','4\n-100\n-90\n',4,[-100,-90]),('boundary','5\n20\n20\n',5,[20,20])]
    invalid={'profile_zero':'0\n-10\n20\n','profile_large':'7\n-10\n20\n','reversed':'3\n20\n-10\n','too_hot':'3\n-10\n101\n','too_cold':'3\n-101\n20\n','malformed':'3\n1x\n20\n','missing':'3\n','empty':'','sign_only':'3\n-\n20\n','long_token':'3\n123456\n20\n','space':'3\n 2\n20\n'}
    manifest={'method':'64 joint endpoint enumeration, exact integers','absolute_tolerance_um':0,'relative_tolerance':0,'cases':[]}
    for name,stdin,profile,temp in cases:
        result,output=enumerate_case(fixtures[profile-1],temp)
        (ROOT/f'data/{name}.stdin').write_text(stdin)
        (ROOT/f'data/{name}.stdout').write_text(output)
        manifest['cases'].append(dict(name=name,exit=0,**result))
    for name,stdin in invalid.items():
        (ROOT/f'data/{name}.stdin').write_text(stdin)
        (ROOT/f'data/{name}.stdout').write_text(HEADER+INVALID)
        manifest['cases'].append(dict(name=name,exit=64))
    manifest['oracle_inputs_sha256']={name:hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in ['data/fixtures.json','reference.py']}
    (ROOT/'data/oracle.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(json.dumps(manifest,indent=2))

if __name__ == '__main__':
    if sys.argv[1:] != ['--freeze']:
        raise SystemExit('usage: reference.py --freeze (never regenerate expectations from compiler output)')
    freeze()
