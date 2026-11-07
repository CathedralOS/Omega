"""Validate actual numerical streams against the frozen independent SVD oracle."""
import argparse, hashlib, json, subprocess
from pathlib import Path
import numpy as np
ROOT=Path(__file__).resolve().parents[1]
def frozen():
 p=ROOT/'validation/frozen.json'
 assert hashlib.sha256(p.read_bytes()).hexdigest()==(p.parent/'FROZEN.sha256').read_text().split()[0]
 manifest=json.loads(p.read_text())
 for name,digest in manifest['input_sha256'].items():
  assert hashlib.sha256((ROOT/'data'/f'{name}.stdin').read_bytes()).hexdigest()==digest,(name,'changed frozen input')
 return manifest
def verify(name,case,stdout):
 lines=stdout.decode('ascii').splitlines();assert lines[0]=='principal_component_analysis',(name,lines)
 fields={line.split()[0]:np.array([float(v) for v in line.split()[1:]]) for line in lines[1:]}
 assert len(fields)==10 and all(np.isfinite(v).all() for v in fields.values()),(name,fields)
 x=np.array(case['x']);n,m=x.shape;v=fields['vectors'].reshape(m,m);scores=fields['scores'].reshape(n,m)
 def close(key,expected):
  observed=fields[key].reshape(np.array(expected).shape);expected=np.array(expected)
  scale=max(np.max(np.abs(expected)),1e-300)
  error=np.max(np.abs(observed-expected))/scale
  assert error<=1e-8,(name,key,error,observed,expected)
 for key in ('means','covariance','matrix','eigenvalues','explained'):close(key,case[key])
 values=fields['eigenvalues'];assert np.all(np.diff(values)<=0),(name,values)
 matrix=np.array(case['matrix']);norm=max(np.linalg.norm(matrix,2),1e-300)
 residual=np.linalg.norm(matrix@v-v*values,ord=2)/norm
 orthogonality=np.linalg.norm(v.T@v-np.eye(m),ord=2)
 assert residual<=1e-8 and orthogonality<=1e-8,(name,residual,orthogonality)
 oracle_v=np.array(case['vectors']);oracle_e=np.array(case['eigenvalues']);start=0
 while start<m:
  end=start+1
  while end<m and abs(oracle_e[end]-oracle_e[start])<=max(oracle_e[0],1e-300)*1e-10:end+=1
  projector=v[:,start:end]@v[:,start:end].T;expected=oracle_v[:,start:end]@oracle_v[:,start:end].T
  assert np.linalg.norm(projector-expected,2)<=1e-8,(name,'subspace',start,end)
  start=end
 scales=np.sqrt(np.diag(case['covariance'])) if case['mode'] else np.ones(m)
 z=(x-np.array(case['means']))/scales
 score_error=np.max(np.abs(scores-z@v))/max(np.max(np.abs(z)),1e-300)
 reconstructed=(scores@v.T)*scales+fields['means']
 reconstruction=np.max(np.abs(reconstructed-x))/max(np.max(np.abs(x)),1e-300)
 assert score_error<=1e-8 and reconstruction<=1e-8,(name,score_error,reconstruction)
 for key,limit in [('residual',1e-8),('orthogonality',1e-8)]:
  assert fields[key].shape==(1,) and 0<=fields[key][0]<=limit,(name,key,fields[key])
 assert fields['reconstruction_error'].shape==(1,) and 0<=fields['reconstruction_error'][0]<=max(np.max(np.abs(z)),1e-300)*1e-8
 return {'residual':float(residual),'orthogonality':float(orthogonality),'score_error':float(score_error),'reconstruction':float(reconstruction)}
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--native',type=Path);parser.add_argument('--terminal',type=Path);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
 assert args.native or args.terminal
 manifest=frozen();out=args.output;out.mkdir(parents=True,exist_ok=True);records=[]
 for name in list(manifest['cases'])+list(manifest['invalid']):
  if args.native:
   result=subprocess.run([str(args.native.resolve())],input=(ROOT/'data'/f'{name}.stdin').read_bytes(),capture_output=True,timeout=20)
   stdout,stderr,exit=result.stdout,result.stderr,result.returncode
  else:
   stdout=(args.terminal/f'{name}.stdout').read_bytes();stderr=(args.terminal/f'{name}.stderr').read_bytes();exit=int((args.terminal/f'{name}.exit').read_text())
  (out/f'{name}.stdout').write_bytes(stdout);(out/f'{name}.stderr').write_bytes(stderr)
  (out/f'{name}.exit').write_text(str(exit)+'\n')
  expected=manifest['invalid'].get(name,0);assert exit==expected,(name,exit,expected,stdout,stderr)
  assert stderr==b'',(name,stderr)
  metrics=verify(name,manifest['cases'][name],stdout) if name in manifest['cases'] else {}
  if expected:
   expected_text={64:b'error invalid input\n',65:b'error constant feature in correlation mode\n',66:b'error numerical range or convergence\n'}[expected]
   assert stdout==expected_text,(name,stdout)
  records.append({'case':name,'exit':exit,**metrics});print(name,'PASS',metrics,flush=True)
 (out/'results.json').write_text(json.dumps({'frozen_sha256':hashlib.sha256((ROOT/'validation/frozen.json').read_bytes()).hexdigest(),'records':records},indent=2)+'\n')
if __name__=='__main__':main()
