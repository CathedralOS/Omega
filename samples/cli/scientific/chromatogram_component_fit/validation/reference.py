"""Independent rational NNLS oracle. No Omega output is read to generate this."""
from fractions import Fraction as F
from pathlib import Path
import json
import sys
ROOT = Path(__file__).resolve().parents[1]
def solve(tokens):
    a,b,y = [list(map(F,tokens[i:i+5])) for i in (0,5,10)]
    profiles = [a,b]
    # Generic rational elimination, unlike the application's 2x2 closed form.
    candidates=[]
    for mask in range(4):
        active=[j for j in range(2) if mask & (1<<j)]
        matrix=[[sum(profiles[j][i]*profiles[k][i] for i in range(5)) for k in active]
                + [sum(profiles[j][i]*y[i] for i in range(5))] for j in active]
        singular=False
        for column in range(len(active)):
            pivot=next((r for r in range(column,len(active)) if matrix[r][column]),None)
            if pivot is None: singular=True; break
            matrix[column],matrix[pivot]=matrix[pivot],matrix[column]
            divisor=matrix[column][column]
            matrix[column]=[v/divisor for v in matrix[column]]
            for row in range(len(active)):
                if row!=column:
                    factor=matrix[row][column]
                    matrix[row]=[v-factor*w for v,w in zip(matrix[row],matrix[column])]
        if singular: continue
        amounts=[F(0),F(0)]
        for row,j in enumerate(active): amounts[j]=matrix[row][-1]
        if min(amounts)<0: continue
        reconstruction=[sum(amounts[j]*profiles[j][i] for j in range(2)) for i in range(5)]
        residuals=[observed-fit for observed,fit in zip(y,reconstruction)]
        sse=sum(v*v for v in residuals)
        candidates.append((sse,mask,amounts,reconstruction,residuals))
    sse,mask,amounts,reconstruction,residuals=min(candidates,key=lambda c:(c[0],c[1]))
    integrated=[sum(profile[1:4])+(profile[0]+profile[4])/2 for profile in profiles]
    determinant=sum(v*v for v in a)*sum(v*v for v in b)-sum(v*w for v,w in zip(a,b))**2
    values={'a_amount':amounts[0],'b_amount':amounts[1], 'a_area':amounts[0]*integrated[0],
            'b_area':amounts[1]*integrated[1], 'sse':sse}
    values.update({f'reconstruction_{i}':v for i,v in enumerate(reconstruction)})
    values.update({f'residual_{i}':v for i,v in enumerate(residuals)})
    return {'mask':mask,'nonidentifiable':determinant==0,'exact':{k:str(v) for k,v in values.items()},
            'numeric':{k:float(v) for k,v in values.items()}}
def freeze():
    data={file.stem:solve(list(map(int,file.read_text().split()))) for file in sorted((ROOT/'data').glob('*.txt')) if not file.name.startswith('invalid_')}
    return {'model':'two profiles, five samples, unit spacing, unweighted NNLS, trapezoidal areas',
            'tolerances':{'reported_absolute':0.000002,'reported_relative':0.000000001,
                          'singular_relative':0.0,'objective_tie_absolute':0.0}, 'cases':data}
if __name__=='__main__':
    if sys.argv[1:] == ['--freeze']:
        destination=ROOT/'validation'/'oracle_v3.json'
        if destination.exists(): raise SystemExit('refusing to overwrite frozen oracle')
        destination.write_text(json.dumps(freeze(),indent=2)+'\n')
    else: print(json.dumps(freeze(),indent=2))
