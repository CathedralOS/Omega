import argparse,hashlib,json,os,subprocess,sys,time
from pathlib import Path

def projects(fact):
 names=subprocess.check_output(['git','ls-files',str(fact)],text=True).splitlines()
 entries=[Path(name) for name in names if name.endswith('/main.omg')]
 positives=[p for p in entries if not any(part in {'invalid','controls','support'} for part in p.relative_to(fact).parts)]
 negatives=[]
 for name in names:
  if name.endswith('/sample.json'):
   metadata=json.loads(Path(name).read_text())
   if metadata.get('role')=='intended-invalid':negatives.append((Path(name).parent/metadata.get('entry','main.omg'),metadata['rejection_contains']))
 return sorted(positives),sorted(negatives)

def main():
 p=argparse.ArgumentParser();p.add_argument('--cli',required=True);p.add_argument('--fact',required=True);p.add_argument('--output',required=True);p.add_argument('--positives',type=int,required=True);p.add_argument('--negatives',type=int,required=True);a=p.parse_args()
 fact=Path(a.fact);positives,negatives=projects(fact)
 if len(positives)!=a.positives or len(negatives)!=a.negatives:raise SystemExit(f'Wrong authored selection: {len(positives)} positives, {len(negatives)} negatives')
 out=Path(a.output);out.mkdir(parents=True,exist_ok=True);observations=[];success=True
 for source,fragments in [(source,None) for source in positives]+negatives:
  command=[a.cli,'--check',str(source)] if fragments else [a.cli,'run','--both','--keep',str(source)]
  started=time.monotonic();r=subprocess.run(command,stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=120);combined=r.stdout+r.stderr
  okay=(r.returncode!=0 and all(fragment in combined for fragment in fragments)) if fragments else (r.returncode==0 and 'native exit 0' in combined and 'interp exit: 0' in combined)
  identity=source.parent.relative_to(fact).as_posix().replace('/','_') or 'root';(out/(identity+'.stdout')).write_text(r.stdout);(out/(identity+'.stderr')).write_text(r.stderr)
  row={'source':str(source),'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'command':command,'exit':r.returncode,'expected_fragments':fragments,'accepted':okay,'seconds':round(time.monotonic()-started,3),'stdout':r.stdout,'stderr':r.stderr};observations.append(row);success &= okay
  print(json.dumps(row),flush=True)
 report={'revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'producer':str(Path(a.cli).resolve()),'producer_sha256':hashlib.sha256(Path(a.cli).read_bytes()).hexdigest(),'target':'linux_x86_64','positives':len(positives),'negatives':len(negatives),'accepted':success,'observations':observations};(out/'observations.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:v for k,v in report.items() if k!='observations'}),flush=True)
 return 0 if success else 1
if __name__=='__main__':raise SystemExit(main())
