"""Collect the completed fixed-source ancestry experiment from the warm worktree."""
import hashlib,json,re,shutil,statistics,subprocess
from pathlib import Path
root=Path.cwd();base=root/'build/source-resolution-remaining';record=root/'wiki/drafts/measurements/source_resolution_remaining_2026_10_29'
def read(p):return json.loads(p.read_text())
def copy(p,q):q.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(p,q)
def stats(rows):
 result={}
 for variant in 'AB':
  subset=[r for r in rows if r['variant']==variant];values=[r['wall_ms'] for r in subset]
  result[variant]={'wall_ms':values,'median_ms':statistics.median(values),'range_ms':[min(values),max(values)],'peak_job_memory_bytes':[r['peak_job_memory_bytes'] for r in subset]}
 result['median_reduction_percent']=100*(1-result['B']['median_ms']/result['A']['median_ms'])
 result['ranges_overlap']=result['B']['range_ms'][1]>=result['A']['range_ms'][0]
 return result
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==(base/'candidate.patch').read_bytes()
summary={'fixed_base':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'attribution':{},'comparison':{},'warm':{}}
native_count=0
for group,directory in [('screen',base),('detail',base/'detail')]:
 rows=read(directory/'screen/runs.json');assert len(rows)==6
 assert all(r['compiler_exit']==0 and r['native_exit']==0 for r in rows)
 for key in ['artifact_sha256','source_sha256','build_sha256']:assert len({r[key] for r in rows})==1
 copy(directory/'screen/runs.json',record/group/'runs.json')
 scopes={}
 for r in rows:
  label=r.get('directory',f"{r['index']:02}-{r['variant']}");p=directory/'screen'/label/'output.txt';copy(p,record/group/(label+'.txt'))
  if r['variant']=='B':
   for line in p.read_text().splitlines():
    if m:=re.search(r'([\d.]+) ms\s+\d+ x\s+(source\.(?:remaining|detail)\..*)',line):scopes.setdefault(m[2],[]).append(float(m[1]))
    if m:=re.match(r'RESOLUTION_DETAIL (\S+) ([\d.]+)',line):scopes.setdefault(m[1],[]).append(float(m[2]))
 summary['attribution'][group]={'variants':stats(rows),'scopes_ms':scopes};native_count+=len(rows)
 for filename in ['diagnostic.patch','build.log']:copy(directory/filename,record/group/filename)
for name in ['diagnose.py','resume.py','detail.py','candidate-chain.py','candidate.patch','candidate-initial-tests.patch','custody-tests-initial-failure.log','custody-tests.log','candidate-build.log','quality.py','quality.log','compare.py','comparison.log','native-replays.json','warm.py']:
 copy(base/name,record/name)
copy(base/'screen/01-B/output.txt',record/'interrupted/01-B-output.txt')
copy(base/'screen/01-B.log',record/'interrupted/01-B-traceback.txt')
replays=read(base/'native-replays.json');assert len(replays)==2 and all(r['exit']==0 for r in replays)
assert len({r['sha256'] for r in replays})==1;native_count+=len(replays)
quality=read(base/'quality/runs.json');assert len(quality)==24
rejects=[r for r in quality if r['label'].endswith('-invalid')];assert len(rejects)==2 and all(r['exit']!=0 and r['matched'] for r in rejects)
native=[r for r in quality if not r['label'].endswith('-invalid')];assert len(native)==22 and all(r['exit']==0 and r['native_exit']==0 for r in native)
assert len([r for r in native if r.get('record_refused')])==8
copy(base/'quality/runs.json',record/'quality/runs.json')
for r in quality:copy(base/'quality'/r['label']/'output.txt',record/'quality'/(r['label']+'.txt'))
native_count+=len(native)
rows=read(base/'comparison/runs.json');metadata=read(base/'comparison/metadata.json')
assert len(rows)==36 and all(r['exit']==0 and r['native_exit']==0 for r in rows)
for producer in metadata['producers'].values():assert hashlib.sha256(Path(producer['path']).read_bytes()).hexdigest()==producer['sha256']
copy(base/'comparison/runs.json',record/'comparison/runs.json');copy(base/'comparison/metadata.json',record/'comparison/metadata.json')
for r in rows:
 label=f"{r['subject']}-{r['index']:02}-{r['variant']}-{r['kind']}";copy(base/'comparison'/label/'output.txt',record/'comparison'/(label+'.txt'))
 assert r['source_cache_depth']==len(Path(r['source_cache_path']).parts)
for subject,source in metadata['sources'].items():
 selected=[r for r in rows if r['subject']==subject];assert len({r['artifact_sha256'] for r in selected})==1
 summary['comparison'][subject]=stats([r for r in selected if r['kind']=='measured'])
 for filename,key in [('main.omg','sha256'),('build.omg','build_sha256')]:
  p=Path(source['path']).with_name(filename);assert hashlib.sha256(p.read_bytes()).hexdigest()==source[key];copy(p,record/'inputs'/subject/filename)
native_count+=len(rows)
warm=read(base/'warm/runs.json');assert len(warm)==24 and all(r['exit']==0 and r['native_exit']==0 for r in warm)
copy(base/'warm/runs.json',record/'warm/runs.json')
for r in warm:
 label=f"{r['subject']}-{r['index']:02}-{r['variant']}";copy(base/'warm'/label/'output.txt',record/'warm'/(label+'.txt'))
for subject in metadata['sources']:
 assert len({r['artifact_sha256'] for r in rows+warm if r['subject']==subject})==1
 summary['warm'][subject]=stats([r for r in warm if r['subject']==subject])
native_count+=len(warm);assert native_count==96
assert '10 tests run: 10 passed' in (base/'custody-tests.log').read_text()
summary.update(native_successes=native_count,compile_native_pairs=94,native_replays=2,static_rejects=2,custody_tests_passed=10,malformed_record_recoveries_included=8,unresolved_native_timeouts=1,cache_depths=sorted({r['source_cache_depth'] for r in rows}))
(record/'summary.json').write_text(json.dumps(summary,indent=2))
print(json.dumps({k:summary[k] for k in ['comparison','warm']},indent=2))
