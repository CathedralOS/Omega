"""Independent oracle: SciPy scalar optimizer, gammaln, ndtr; never Omega output."""
import json, re
from pathlib import Path
import numpy as np
from scipy.optimize import brentq
from scipy.special import gammaln, ndtr
ROOT = Path(__file__).resolve().parents[1]

def analyze(text):
    if len(text.encode('utf-8')) >= 600 or re.search(r'[^0-9\- \t\r\n]',text):
        return {'exit':64,'message':'INVALID syntax'}
    tokens=text.split()
    if any(not re.fullmatch(r'-?[0-9]+',token) for token in tokens):
        return {'exit':64,'message':'INVALID syntax'}
    try:
        values = list(map(int, tokens))
        if any(abs(value)>1000009 for value in values): return {'exit':64,'message':'INVALID syntax'}
        if len(values) != 54: return {'exit':64, 'message':'INVALID syntax'}
    except ValueError: return {'exit':64, 'message':'INVALID syntax'}
    groups = np.array(values[:6]); counts = np.array(values[6:]).reshape(8,6)
    if any((groups < 0)|(groups > 1)) or not 2 <= sum(groups) <= 4 or np.any((counts < 0)|(counts > 1000)):
        return {'exit':65,'message':'INVALID metadata_or_count_range'}
    positive = counts[np.all(counts > 0, axis=1)]
    if not len(positive): return {'exit':66,'message':'EXCLUDED normalization_no_positive_gene'}
    factors = np.exp(np.median(np.log(positive) - np.mean(np.log(positive),axis=1)[:,None],axis=0))
    normalized = counts/factors
    rows=[]
    for y,n in zip(counts,normalized):
        means=np.array([np.mean(n[groups==g]) for g in [0,1]])
        reason = 'all_zero' if sum(y)==0 else 'low_total_count' if sum(y)<10 else 'group_zero_boundary' if min(means)==0 else None
        if reason: rows.append({'reason':reason}); continue
        residual=n-means[groups]
        alpha=float(np.clip(np.sum(residual**2-means[groups]/factors)/(4/6*np.sum(means[groups]**2)),1e-6,10))
        fitted=[]; information=[]
        for g in [0,1]:
            selected=groups==g
            score=lambda mean: np.sum((y[selected]-factors[selected]*mean)/(1+alpha*factors[selected]*mean))
            mean=brentq(score,1e-10,1e6,xtol=1e-10)
            fitted.append(mean)
            mu=factors[selected]*mean
            information.append(np.sum(mu/(1+alpha*mu)))
        fold=np.log2(fitted[1]/fitted[0]); error=np.sqrt(sum(1/np.array(information)))/np.log(2)
        mu=factors*np.array(fitted)[groups]; r=1/alpha
        likelihood=np.sum(gammaln(y+r)-gammaln(r)-gammaln(y+1)+r*np.log(r/(r+mu))+y*np.log(mu/(r+mu)))
        rows.append({'reason':'tested','values':[float(np.mean(n)),alpha,*fitted,float(fold),float(error),float(likelihood),float(2*ndtr(-abs(fold/error))),0.]})
    tested=[i for i,row in enumerate(rows) if row['reason']=='tested']
    order=sorted(tested,key=lambda i: rows[i]['values'][7]); best=1.
    for rank,i in reversed(list(enumerate(order,1))):
        best=min(best,rows[i]['values'][7]*len(order)/rank); rows[i]['values'][8]=best
    return {'exit':0,'factors':factors.tolist(),'rows':rows}

if __name__=='__main__':
    expected={p.stem:analyze(p.read_text()) for p in sorted((ROOT/'data').glob('*.stdin'))}
    (ROOT/'validation/oracle.json').write_text(json.dumps(expected,indent=2)+'\n')
