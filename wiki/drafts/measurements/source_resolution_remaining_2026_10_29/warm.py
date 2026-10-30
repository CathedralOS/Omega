import os,json,sys,subprocess
from pathlib import Path
root=Path.cwd();base=root/'build/source-resolution-remaining';out=base/'warm';out.mkdir(exist_ok=False)
sys.path.insert(0,str(root/'tools/benchmark'));import benchmark
prior=json.loads((base/'comparison/runs.json').read_text());metadata=json.loads((base/'comparison/metadata.json').read_text());rows=[]
for r in prior:
 if r['kind']!='measured':continue
 v=r['variant'];subject=r['subject'];label=f"{subject}-{r['index']:02}-{v}"
 producer=Path(metadata['producers'][v]['path']);assert benchmark.sha256_file(producer)==metadata['producers'][v]['sha256']
 original=base/'comparison'/f"{subject}-{r['index']:02}-{v}-measured";project=original/'input';cache=Path(r['source_cache_path'])
 for file,key in [('main.omg','sha256'),('build.omg','build_sha256')]:assert benchmark.sha256_file(project/file)==metadata['sources'][subject][key]
 before=[{'relative_path':str(p.relative_to(cache)),'bytes':p.stat().st_size,'sha256':benchmark.sha256_file(p)} for p in cache.rglob('*.rec')];assert before
 directory=out/label;directory.mkdir();os.environ['LOCALAPPDATA']=str(cache);os.environ.pop('OMEGA_PHASE_PROFILE',None)
 command=[str(producer),'--timings','--target','windows_x86_64','--build-dir',str(directory/'build'),str(project/'main.omg')]
 code,wall,memory,text=benchmark.capture_compile(command,root);(directory/'output.txt').write_text(text,encoding='utf-8');stages,artifact=benchmark.parse_timings(text)
 row={'subject':subject,'index':r['index'],'variant':v,'command':command,'exit':code,'wall_ms':wall,'peak_job_memory_bytes':memory,'stages':stages,'producer_sha256':benchmark.sha256_file(producer),'source_sha256':benchmark.sha256_file(project/'main.omg'),'build_sha256':benchmark.sha256_file(project/'build.omg'),'records_before':before};rows.append(row);(out/'runs.json').write_text(json.dumps(rows,indent=2))
 assert code==0 and artifact and Path(artifact).is_file(),text[-2000:]
 native=subprocess.run([artifact],capture_output=True,timeout=30);row.update(native_exit=native.returncode,artifact_sha256=benchmark.sha256_file(artifact),stdout_hex=native.stdout.hex(),stderr_hex=native.stderr.hex());(out/'runs.json').write_text(json.dumps(rows,indent=2))
 assert native.returncode==0 and row['artifact_sha256']==r['artifact_sha256']
 print(label,round(wall,1),'ms; native verified',flush=True)
assert len(rows)==24
