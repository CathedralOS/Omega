from pathlib import Path
import json,hashlib,numpy as np
root=Path(__file__).resolve().parents[1]
a=np.array([[2,1,5],[4,3,2],[1,4,3],[5,2,7],[3,6,4]],float)
cases={}
def add(name,x,mode=0):
 x=np.array(x,float);n,m=x.shape;means=x.mean(0);center=x-means;cov=(center/np.sqrt(n-1)).T@(center/np.sqrt(n-1));scale=np.sqrt(np.diag(cov)) if mode else np.ones(m);z=center/scale
 u,s,v=np.linalg.svd(z,full_matrices=True);e=np.zeros(m);e[:len(s)]=(s/np.sqrt(n-1))**2
 cases[name]={'mode':mode,'x':x.tolist(),'means':means.tolist(),'covariance':cov.tolist(),'matrix':((z/np.sqrt(n-1)).T@(z/np.sqrt(n-1))).tolist(),'eigenvalues':e.tolist(),'vectors':v.T.tolist(),'explained':((e/e[0])/(e/e[0]).sum() if e[0] else e).tolist()}
 payload=f'{mode} {n} {m} '+' '.join(format(t,'.17g') for t in x.flat)+'\n';(root/'data'/f'{name}.stdin').write_text(payload)
add('covariance',a);add('correlation',a,1);add('translated',a+[10,-20,100]);add('permuted',a[:,[2,0,1]]);add('rank_one',[[1,2,3,4],[2,4,6,8],[3,6,9,12]]);add('constant',np.ones((3,2)));add('repeated',[[1,0],[-1,0],[0,1],[0,-1]]);add('repeated_four',np.concatenate([np.eye(4),-np.eye(4)]));add('maximum',np.random.default_rng(4101).normal(size=(16,4)));add('scaled_small',a*1e-5);add('scaled_large',a*1e5);add('minimum',[[1,2],[3,1],[2,4]],1);add('large_trace',[[1.2e154,0],[-1.2e154,0],[0,1.2e154],[0,-1.2e154]]);add('rank_correlation',[[1,2,3,4],[2,4,6,8],[3,6,9,12]],1);add('decimal_constant',[[1.234,2.345]]*7)
invalid={'numerical_range':'0 3 2 1e308 1 -1e308 2 0 3','constant_correlation':'1 3 2 1 0 1 2 1 4','nan':'0 3 2 1 2 NaN 4 5 6','infinity':'0 3 2 1 2 inf 4 5 6','overflow':'0 3 2 1e309 2 3 4 5 6','few_rows':'0 2 2 1 2 3 4','many_rows':'0 17 2','few_features':'0 3 1','many_features':'0 3 5','bad_mode':'2 3 2','fraction_dimension':'0 3.5 2','junk':'0 3 2 1 2 3x 4 5 6','truncated':'0 3 2 1 2 3 4 5','extra':'0 3 2 1 2 3 4 5 6 7','empty':'','long_token':'0 3 2 '+'1'*80,'bytes':' '*4097,'bad_exponent':'0 3 2 1e 2 3 4 5 6','double_dot':'0 3 2 1..2 2 3 4 5 6'}
for name,p in invalid.items():(root/'data'/f'{name}.stdin').write_text(p+'\n')
frozen={'oracle':'NumPy centered data SVD, independently authored before sample execution','numpy':np.__version__,'input_sha256':{p.stem:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((root/'data').glob('*.stdin'))},'cases':cases,'invalid':{n:65 if n=='constant_correlation' else 66 if n=='numerical_range' else 64 for n in invalid}}
(root/'validation/frozen.json').write_text(json.dumps(frozen,indent=2)+'\n')
h=hashlib.sha256((root/'validation/frozen.json').read_bytes()).hexdigest();(root/'validation/FROZEN.sha256').write_text(h+'  frozen.json\n')
(root/'input.txt').write_bytes((root/'data/covariance.stdin').read_bytes())
