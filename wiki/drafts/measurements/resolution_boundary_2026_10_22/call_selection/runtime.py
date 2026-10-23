import hashlib,json,os,subprocess,sys,shutil
from pathlib import Path
sys.path.insert(0,str(Path('tools/benchmark').resolve()))
from benchmark import parse_timings,require_optimized
folder=Path(sys.argv[1]); mode=sys.argv[2]
binary=(folder/'omega-instrumented').resolve();require_optimized(binary)
subjects=[
 ('returned','tests/omega/pass/layouts/placed_reference_results/returned/main.omg'),
 ('forwarded','tests/omega/pass/layouts/placed_reference_results/forwarded/main.omg'),
 ('length','samples/cli/facts/length/local/main.omg'),
 ('dyn','samples/cli/facts/dyn_dispatch_selector_payload_bound/local/main.omg'),
 ('record','samples/cli/facts/nested_nine_level_record_field_bound/local/main.omg'),
 ('target_return','build/placed-result-selection/witnesses/target_return/main.omg'),
 ('conformance_return','build/placed-result-selection/witnesses/conformance_return/main.omg'),
 ('projected_receiver','build/placed-result-selection/witnesses/projected_receiver/main.omg'),
 ('nested','tests/omega/pass/layouts/placed_reference_results/nested/main.omg'),
]
environment=os.environ.copy();environment.pop('OMEGA_CALL_RESULT_EXPERIMENT',None)
if mode=='project':environment['OMEGA_CALL_RESULT_EXPERIMENT']='project'
record=dict(mode=mode,producer=str(binary),sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),observations=[])
def run(command):
 result=subprocess.run(command,env=environment,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
 row=dict(command=command,exit=result.returncode,output=result.stdout);return row
for name,subject in subjects:
 subject=str(Path(subject).resolve());products=(folder/'products'/name).resolve();products.mkdir(parents=True,exist_ok=True)
 row=dict(name=name,subject=subject)
 row['compile']=run([str(binary),'--timings','--target','macos_arm64','--build-dir',str(products),subject])
 _,published=parse_timings(row['compile']['output'])
 if row['compile']['exit']==0 and published:
  path=Path(published);image=folder/('native-'+name);shutil.copy2(path,image)
  row['native_sha256']=hashlib.sha256(image.read_bytes()).hexdigest();row['native']=run([str(image.resolve())])
 row['both']=run([str(binary),'run','--both',subject])
 record['observations'].append(row)
 (folder/'runtime.json').write_text(json.dumps(record,indent=2)+'\n')
 print(name,'compile',row['compile']['exit'],'native',row.get('native',{}).get('exit'),'both',row['both']['exit'],flush=True)
