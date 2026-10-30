import json,os,re,shutil,subprocess,sys
from pathlib import Path
root=Path.cwd();out=root/'build/source-resolution/quality';out.mkdir(exist_ok=True)
sys.path.insert(0,str(root/'tools/benchmark'));import benchmark
producers={'A':root/'build/flow-evaluation/candidate/omega.exe','B':root/'build/source-resolution/candidate/omega.exe'}
subjects={'stack32':'thirtytwo_arg_stack_calling_plan/other_machine','length':'length/local','dispatch':'dyn_dispatch_selector_payload_bound/match_join'}
rows=json.loads((out/'runs.json').read_text()) if (out/'runs.json').exists() else [];expected={};length_context={}
def save(): (out/'runs.json').write_text(json.dumps(rows,indent=2),encoding='utf-8')
def run(label,producer,project,cache):
 old=next((row for row in rows if row['label']==label),None)
 if old is not None:return old,(out/label/'output.txt').read_text()
 os.environ['LOCALAPPDATA']=str(cache);os.environ['OMEGA_PHASE_PROFILE']='1'
 directory=out/label;directory.mkdir()
 command=[str(producer),'--timings','--target','windows_x86_64','--build-dir',str(directory/'build'),str(project/'main.omg')]
 code,wall,memory,text=benchmark.capture_compile(command,root);(directory/'output.txt').write_text(text,encoding='utf-8')
 row={'label':label,'command':command,'producer_sha256':benchmark.sha256_file(producer),'source_sha256':benchmark.sha256_file(project/'main.omg'),'exit':code,'wall_ms':wall,'peak_job_memory_bytes':memory}
 rows.append(row);save();_,artifact=benchmark.parse_timings(text)
 assert code==0 and artifact and Path(artifact).is_file(),text[-2500:]
 native=subprocess.run([artifact],capture_output=True,timeout=30)
 row.update(native_exit=native.returncode,artifact_sha256=benchmark.sha256_file(artifact),stdout_hex=native.stdout.hex(),stderr_hex=native.stderr.hex());save();assert native.returncode==0
 print(label,'native verified',flush=True)
 return row,text
for variant,producer in producers.items():
 benchmark.require_optimized(producer)
 for subject,relative in subjects.items():
  directory=out/(subject+'-'+variant);project=directory/'input';project.mkdir(parents=True,exist_ok=True);cache=directory/'cache';cache.mkdir(exist_ok=True)
  source=root/'samples/cli/facts'/relative
  for filename in ['main.omg','build.omg']:shutil.copyfile(source/filename,project/filename)
  os.environ['LOCALAPPDATA']=str(cache);os.environ.pop('OMEGA_PHASE_PROFILE',None)
  if not any(row['label']==subject+'-'+variant+'-first' for row in rows):benchmark.settle_package_review(producer,project,'windows_x86_64')
  first,text=run(subject+'-'+variant+'-first',producer,project,cache)
  warm,warm_text=run(subject+'-'+variant+'-warm',producer,project,cache)
  assert first['artifact_sha256']==warm['artifact_sha256']
  if variant=='A':expected[subject]=first['artifact_sha256']
  else:
   assert expected[subject]==first['artifact_sha256']
   assert 'source.graph.cold' in text and 'source.graph.loaded' in warm_text
   assert 'source.graph.prepared_hits' in warm_text and '05.b.retained_match.hit' in warm_text
  record=Path(re.search(r'^compiler source record: (.+)$',warm_text,re.M)[1].strip())
  candidates=[p for p in cache.rglob('*.rec') if p.name==record.name and p.samefile(record)]
  assert len(candidates)==1
  record=candidates[0]
  assert record.resolve().is_relative_to(cache.resolve())
  if subject=='length':length_context[variant]=(producer,project,cache,record)
for variant,(producer,project,cache,record) in length_context.items():
 original=record.read_bytes();original_hash=benchmark.sha256_file(record)
 for kind in ['truncated','foreign-compiler','checksum','oversized']:
  record.write_bytes(original)
  if kind=='truncated':
   with record.open('r+b') as stream:stream.truncate(64)
  elif kind=='oversized':
   with record.open('r+b') as stream:stream.truncate(512*1024*1024+81)
  else:
   with record.open('r+b') as stream:
    stream.seek(8 if kind=='foreign-compiler' else 40);value=stream.read(1);stream.seek(-1,1);stream.write(bytes([value[0]^1]))
  mutation={'kind':kind,'original_sha256':original_hash,'bytes':record.stat().st_size}
  row,text=run('length-'+variant+'-'+kind,producer,project,cache)
  row['mutation']=mutation;row['record_refused']='source.graph.invalid' in text and 'retained source graph ignored:' in text;save()
  assert row['record_refused'] and row['artifact_sha256']==expected['length']
 # Change source under the same live entry and refresh through ordinary review.
 source=project/'main.omg';old=source.read_text();new=old.replace('[4, 7, 8, 9]','[4, 7, 11, 9]').replace('== 8','== 11');assert new!=old
 source.write_text(new);os.environ.pop('OMEGA_PHASE_PROFILE',None);benchmark.settle_package_review(producer,project,'windows_x86_64')
 edited,text=run('length-'+variant+'-edited',producer,project,cache)
 assert edited['artifact_sha256']!=expected['length']
 if variant=='A':edited_hash=edited['artifact_sha256']
 else:assert edited['artifact_sha256']==edited_hash
 # A fresh invalid source at that same path must not inherit the cached judgment.
 invalid=new.replace('last_byte(&items[..], 3)','last_byte(&items[..], 0)');assert invalid!=new;source.write_text(invalid)
 os.environ.pop('OMEGA_PHASE_PROFILE',None);command=[str(producer),'update','--project',str(project),'--target','windows_x86_64'];code,text=benchmark.run_plain(command,root)
 directory=out/('length-'+variant+'-invalid');directory.mkdir();(directory/'output.txt').write_text(text,encoding='utf-8')
 row={'label':'length-'+variant+'-invalid','command':command,'producer_sha256':benchmark.sha256_file(producer),'source_sha256':benchmark.sha256_file(source),'exit':code,'required_diagnostic':'count > 0','matched':'count > 0' in text};rows.append(row);save()
 assert code!=0 and row['matched'],text[-2500:]
 print(variant,'edited source and invalid contract verified',flush=True)
assert len(rows)==24
print('Warm reuse, eight malformed records, edited sources and invalid contracts passed.',flush=True)
