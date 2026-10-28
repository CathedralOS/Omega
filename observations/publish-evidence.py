from pathlib import Path
import gzip,json,hashlib,os,subprocess,sys
sys.path.insert(0,'tools')
from corpus_gate import retain_record_source
r=Path('build/wrapping-integration');state_path=Path('build/private-index-probe/qualification.json');s=json.loads(state_path.read_text());head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
assert s['running'] is None and s['runs'][-1]['revision']==head and s['runs'][-1]['exit']==0 and not s['runs'][-1]['invalidations']
assert s['base']=='baf1b6ea0633681bcc2cf34d563d748a47fadb36'
files={}
for source,name in [(state_path,'state.json.gz'),(r/'qualification-config.json','config.json.gz')]:
 p=r/name
 with gzip.open(p,'wb',compresslevel=1) as f:f.write(source.read_bytes())
 files['qualification/'+name]=p
for i,run in enumerate(s['runs']):
 p=r/('receipt-'+str(i)+'.json.gz')
 with gzip.open(p,'wb',compresslevel=1) as f:f.write(Path(run['receipt']['path']).read_bytes())
 files['qualification/'+p.name]=p
 for stream in ['stdout','stderr']:files['qualification/run-'+str(i)+'-'+stream+'.txt']=Path(run[stream]['path'])
for p in r.iterdir():
 if p.is_file() and p.suffix in ['.json','.log','.py','.txt','.jsonl'] and p.name not in ['evidence-manifest.json','evidence-publication.json']:
  if p.stat().st_size>2_000_000:
   dest=r/(p.name+'.gz')
   with gzip.open(dest,'wb',compresslevel=1) as f:f.write(p.read_bytes())
   files['observations/'+dest.name]=dest
  else:files['observations/'+p.name]=p
for n in ['main.omg','build.omg','harness.omg']:files['parser-probe/'+n]=r/'parser-probe'/n
for n in ['main.omg','build.omg']:files['counterfactual/'+n]=Path('build/private-index-probe/counterfactual')/n
assert retain_record_source(head) is None
files['source/'+head+'.bundle']=Path('build/corpus-source')/(head+'.bundle')
manifest=dict(submitted_head=head,fixed_base=s['base'],integration=json.loads((r/'integration.json').read_text()),original_evidence='287984790a98438a98977526226971e99b7f8527 at work/nested-wrapping-index-evidence retains original baseline, original b7dd acceptance, and raw original observations with actual producers14f/1dd and baseline485.',acceptance=json.loads((r/'acceptance.json').read_text()),files=[dict(path=n,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for n,p in files.items()]);(r/'evidence-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');files['manifest.json']=r/'evidence-manifest.json'
env=dict(os.environ,GIT_INDEX_FILE=str((r/'evidence.index').resolve()));subprocess.run(['git','read-tree','--empty'],env=env,check=True)
for name,path in files.items():
 blob=subprocess.check_output(['git','hash-object','-w',str(path)],text=True).strip();subprocess.run(['git','update-index','--add','--cacheinfo',f'100644,{blob},{name}'],env=env,check=True)
tree=subprocess.check_output(['git','write-tree'],env=env,text=True).strip();commit=subprocess.check_output(['git','commit-tree',tree],input='psi: retain integrated finite arithmetic execution evidence\n',text=True).strip();ref='work/nested-wrapping-index-integrated-evidence';subprocess.run(['git','push','origin',f'{commit}:refs/heads/{ref}'],check=True);(r/'evidence-publication.json').write_text(json.dumps(dict(commit=commit,ref=ref),indent=2)+'\n');print(commit)
