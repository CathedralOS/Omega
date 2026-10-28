from pathlib import Path
import json,subprocess,hashlib,os,time
r=Path('build/wrapping-integration'); old=Path('build/private-index-probe'); revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
def digest(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def invoke(command,name,timeout=600,env=None):
 started=time.monotonic()
 with (r/(name+'.log')).open('w') as f:
  try: x=subprocess.run(command,stdout=f,stderr=subprocess.STDOUT,timeout=timeout,env=env);code=x.returncode
  except subprocess.TimeoutExpired:code='timeout'
 result=dict(command=command,exit=code,elapsed_seconds=time.monotonic()-started,revision=revision)
 (r/(name+'-command.json')).write_text(json.dumps(result,indent=2)+'\n'); print(name,code,round(result['elapsed_seconds'],2),flush=True);return code
for source,name in [('build-cli.jsonl','omega'),('build-runner.jsonl','corpus_runner')]:
 artifacts=[]
 for line in (r/source).read_text().splitlines():
  try:x=json.loads(line)
  except json.JSONDecodeError:continue
  if x.get('reason')=='compiler-artifact' and x.get('executable') and x['target']['name']==name:artifacts.append(x)
 assert artifacts,(source,name)
 artifact=artifacts[-1];path=r/'producer'/name
 import shutil
 shutil.copy2(artifact['executable'],path)
 profile=subprocess.check_output([str(path.resolve()),'--build-profile'],text=True)
 assert json.loads(profile)['opt_level']=='1'
 (r/(name+'-producer.json')).write_text(json.dumps(dict(revision=revision,sha256=digest(path),profile=profile,cargo_artifact=artifact),indent=2)+'\n')
cli=str((r/'producer/omega').resolve());runner=str((r/'producer/corpus_runner').resolve())
for script,name in [('run-row.py','row'),('run-controls.py','controls')]:
 text=(old/script).read_text().replace("Path('build/private-index-probe/row-'+label+'.json')","Path('build/wrapping-integration/row.json')").replace("Path('build/private-index-probe/controls-'+label+'.json')","Path('build/wrapping-integration/controls.json')")
 (r/script).write_text(text)
 assert invoke(['python3',str(r/script),cli,'integrated','both'],name)==0
p=old/'counterfactual/main.omg';command=[cli,'run','--both',str(p)];x=subprocess.run(command,input='',capture_output=True,text=True,timeout=180)
(r/'counterfactual.json').write_text(json.dumps(dict(command=command,sha256=digest(p),exit=x.returncode,stdout=x.stdout,stderr=x.stderr),indent=2)+'\n')
# Fresh affected scope: all captured finite policies, indexed private cycles,
# qualified endpoints and readable/write-only field crossings. The previous
# larger scope remains historical evidence with its original producer identity.
original=(old/'focused-filter.txt').read_text().strip().split(',')
filters=['facts/nested_machine_wrapping_index_','facts/nested_machine_private_cyclic_','qualified_field','write_only_field','write_only_qualified','write_only_stored_reference','borrowed_projection']
filters.extend(f for f in original if ('wrapping' in f or 'saturating' in f) and 'nested_machine' not in f)
(r/'focused-filter.txt').write_text(','.join(filters)+'\n')
for mode in ['source','both']:
 command=['python3','tools/corpus_gate.py','--runner',runner,'--filter',','.join(filters),'--json',str(r/('corpus-'+mode+'.json'))]
 if mode=='both':command+=['--both']
 invoke(command,'corpus-'+mode,900,dict(os.environ,OMEGA_CORPUS_JOBS='4'))
paths=subprocess.check_output(['git','diff','--name-only','eacbda4ec3ee458c8a870b54b66161dae55621d2','HEAD','--','omega-rust'],text=True).splitlines()
assert invoke(['rustfmt','--check','--edition','2024','--config','skip_children=true',*[p for p in paths if p.endswith('.rs')]],'format')==0
assert invoke(['cargo','clippy','--locked','-p','validation','-p','typed-trees-to-checked-trees','--no-deps'],'clippy',900)==0

# Preserve earlier accepted/generated state; replay in a new authored project.
probe=r/'parser-probe';probe.mkdir(exist_ok=True)
for name in ['main.omg','build.omg','harness.omg']:
 (probe/name).write_bytes((Path('build/product-nested/parser-baseline')/name).read_bytes())
invoke(['sh','tests/product/gate.sh','parser',str(probe),'tests/psi/parser/test_parser.py'],'parser-admission',180,dict(os.environ,OMEGA_CLI=cli,OMEGA_TARGET='macos_arm64'))
