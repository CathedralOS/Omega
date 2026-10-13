"""Validate native AND actual captured retained Terminal numerical streams."""
import argparse,hashlib,json,math,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
REPOSITORY=ROOT.parents[3]
ORACLE_SHA='be84d7243b0a809b2d5d8729625a3d6dfae965bc55d025d940f913fcb14f1744'
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def run(command,stdin,output,stem,timeout=120):
 result=subprocess.run(command,input=stdin,capture_output=True,timeout=timeout)
 for suffix,data in [('stdout',result.stdout),('stderr',result.stderr),('exit',str(result.returncode).encode())]:output.joinpath(stem+'.'+suffix).write_bytes(data)
 return result

def check(name,stdout,stderr,exit,oracle):
 case=oracle['cases'][name];assert exit==case['exit'],(name,exit,stdout,stderr)
 assert not stderr,(name,stderr)
 if exit:
  assert stdout==(b'error invalid input\n' if exit==64 else b'status singular-system\n'),(name,stdout)
  return {'case':name,'exit':exit,'oracle':'matched'}
 lines=stdout.decode('ascii').splitlines();assert lines[0]=='status ordinary-kriging; decimal scientific tokens',lines
 tokens=ROOT.joinpath('data',name+'.stdin').read_text().split();n,m=map(int,tokens[:2]);v=float(tokens[2]);values=[float(tokens[7+3*i]) for i in range(n)]
 assert len(lines)==1+2*m,(name,lines)
 rows=[];error=0
 for q,expected in enumerate(case['expected']['queries']):
  row=lines[1+2*q].split();weights=lines[2+2*q].split()
  assert row[0]=='query' and len(row)==5 and weights[0]=='weights' and len(weights)==n+1
  actual=[float(x) for x in row[1:]];w=[float(x) for x in weights[1:]]
  assert all(math.isfinite(x) for x in actual+w)
  prediction,variance,system,mean=actual
  errors=[abs(prediction-float(expected['prediction'])),abs(variance-float(expected['variance']))]+[abs(a-float(b)) for a,b in zip(w,expected['weights'])]
  error=max(error,*errors);assert max(errors)<=1e-7,(name,q,errors)
  assert abs(sum(w)-1)<=1e-9 and 0<=system<=1e-9 and 0<=mean<=1e-9,(name,q,actual,w)
  assert abs(mean-abs(sum(w)-1))<=1e-10
  assert abs(prediction-sum(a*b for a,b in zip(w,values)))<=1e-7
  cross=[float(c)/v for c in expected['cross']];matrix=[[float(c)/v for c in line] for line in case['expected']['covariance']]
  multiplier=1-sum(a*b for a,b in zip(w,cross))-variance/v
  residual=max(abs(sum(a*b for a,b in zip(line,w))+multiplier-c)/(1+sum(abs(a*b) for a,b in zip(line,w))+abs(multiplier)+abs(c)) for line,c in zip(matrix,cross))
  assert residual<=1e-9,(name,q,residual)
  assert abs(system-residual)<=1e-9
  # Independently reconstruct latent MSE, not noisy-target or simple kriging.
  mse=v-2*v*sum(a*b for a,b in zip(w,cross))+v*sum(w[i]*matrix[i][j]*w[j] for i in range(n) for j in range(n))
  assert abs(variance-mse)<=1e-7 and variance>=-1e-10,(name,q,variance,mse)
  rows.append({'weights':w,'prediction':prediction,'variance':variance,'scaled_residual':residual})
 return {'case':name,'exit':exit,'maximum_error':error,'queries':rows}
def invariants(results):
 rows={x['case']:x for x in results}
 for name in ('translated','permuted','eof','tabs','crlf'):
  for original,other in zip(rows['planar']['queries'],rows[name]['queries']):
   assert abs(original['prediction']-other['prediction'])<=1e-9 and abs(original['variance']-other['variance'])<=1e-9
   expected=original['weights'] if name!='permuted' else [original['weights'][i] for i in [2,0,3,1]]
   assert max(abs(a-b) for a,b in zip(expected,other['weights']))<=1e-9
 for q,row in enumerate(rows['interpolation']['queries']):
  assert abs(row['variance'])<=1e-9
  assert max(abs(w-(1 if q==i else 0)) for i,w in enumerate(row['weights']))<=1e-9
 for row in rows['constant']['queries']:assert abs(row['prediction']-7)<=1e-9
 assert abs(rows['noisy_duplicates']['queries'][0]['weights'][0]-rows['noisy_duplicates']['queries'][0]['weights'][1])<=1e-9

def main():
 p=argparse.ArgumentParser();p.add_argument('--omega',required=True,type=Path);p.add_argument('--route',choices=['all','native','terminal','invalid'],default='all')
 p.add_argument('--build-json',type=Path,default=REPOSITORY/'build/w2-24/cargo.json');p.add_argument('--output',type=Path,default=ROOT/'build/validation');args=p.parse_args()
 output=args.output.resolve();output.mkdir(parents=True,exist_ok=True);omega=args.omega.resolve()
 assert digest(ROOT/'validation/frozen.json')==ORACLE_SHA,'frozen oracle changed'
 oracle=json.loads(ROOT.joinpath('validation/frozen.json').read_text())
 for name,case in oracle['cases'].items():assert digest(ROOT/'data'/f'{name}.stdin')==case['input_sha256'],name
 names=output/'cases.txt';names.write_text('\n'.join(oracle['cases'])+'\n');results={}
 if args.route in ('all','terminal','native'):
  observer=output/'terminal-sponsor'
  link=run([sys.executable,str(ROOT/'validation/link_terminal.py'),str(args.build_json.resolve()),str(REPOSITORY),str(observer)],b'',output,'link',180)
  assert link.returncode==0,link.stderr.decode(errors='replace')
  receipt=json.loads(observer.with_suffix('.link.json').read_text());assert receipt['cli_binary_sha256']==digest(omega),'sponsor and CLI producer disagree'
  captured=output/'terminal';captured.mkdir(exist_ok=True)
  observed=run([str(observer),str(ROOT),str(names),str(captured)],b'',output,'terminal-sponsor',1800)
  assert observed.returncode==0,observed.stdout.decode(errors='replace')+observed.stderr.decode(errors='replace')
  if args.route in ('all','terminal'):
   results['terminal']=[check(name,(captured/(name+'.stdout')).read_bytes(),(captured/(name+'.stderr')).read_bytes(),int((captured/(name+'.exit')).read_text()),oracle) for name in oracle['cases']]
   invariants(results['terminal'])
  if args.route in ('all','native'):
   binary=Path((captured/'native.path').read_text());assert binary.is_file()
   results['native']=[]
   for name in oracle['cases']:
    actual=run([str(binary)],(ROOT/'data'/f'{name}.stdin').read_bytes(),output,name+'.native',30)
    results['native'].append(check(name,actual.stdout,actual.stderr,actual.returncode,oracle))
   invariants(results['native'])
   results['native_binary_sha256']=digest(binary)
 if args.route in ('all','invalid'):
  source=ROOT/'controls/covariance_index/main.omg'
  actual=run([str(omega),'--check','--target','linux_x86_64',str(source)],b'',output,'source-invalid',120)
  message=(actual.stdout+actual.stderr).decode(errors='replace')
  assert actual.returncode==1 and 'index' in message and '64' in message,message
  assert all(word not in message.lower() for word in ['custody','package review','parse error','unresolved']),message
  results['source_invalid']={'exit':actual.returncode,'diagnostic':message}
 results.update({'oracle_sha256':ORACLE_SHA,'omega_sha256':digest(omega),'revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPOSITORY,text=True).strip(),'source_hashes':{str(f.relative_to(ROOT)):digest(f) for f in ROOT.rglob('*.omg') if 'build' not in f.relative_to(ROOT).parts}})
 output.joinpath('results.json').write_text(json.dumps(results,indent=2)+'\n')
 print(args.route+': actual numerical and refusal oracles matched; '+str(len(oracle['cases']))+' fixtures')
if __name__=='__main__':main()
