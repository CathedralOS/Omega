import hashlib,json,subprocess,sys,shutil
from pathlib import Path
sys.path.insert(0,str(Path('tools/benchmark').resolve()))
from benchmark import parse_timings
out=Path('build/deferred-completion-storage-final');binary=(out/'omega-candidate').resolve()
rows=json.loads((out/'baseline-observations.json').read_text())['observations'];record=[]
for row in rows:
 if not row['native']:continue
 products=(out/'products'/row['name']).resolve();products.mkdir(parents=True,exist_ok=True)
 command=[str(binary),'--timings','--target','macos_arm64','--build-dir',str(products),str(Path(row['subject']).resolve())]
 result=subprocess.run(command,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
 _,published=parse_timings(result.stdout)
 assert result.returncode==0 and published,(command,result.stdout)
 image=out/('native-'+row['name']);assert not image.exists(),image
 shutil.copy2(published,image)
 native=subprocess.run([str(image.resolve())],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=30)
 assert native.returncode==0,(image,native)
 record.append(dict(name=row['name'],command=command,compile_exit=result.returncode,compile_output=result.stdout,path=str(image.resolve()),sha256=hashlib.sha256(image.read_bytes()).hexdigest(),native_exit=native.returncode,native_output=native.stdout))
 (out/'products.json').write_text(json.dumps(record,indent=2)+'\n')
 print(row['name'],'native image frozen and executed',flush=True)
