"""Capture native and admitted Terminal stdout with the documented corpus harness.
Temporary complete source fixtures are copies of the frozen application, not
constant-result witnesses. One fixture runs at a time; no expected data is written.
"""
import argparse,hashlib,json,os,pathlib,shutil,subprocess,sys,tempfile
p=argparse.ArgumentParser();p.add_argument('--runner',required=True);a=p.parse_args()
app=pathlib.Path(__file__).resolve().parent;repo=app.parents[3]
fixture_parent=repo/'tests/omega/run/calls/new070_sensor_stdout_validation'
assert not fixture_parent.exists(),'temporary fixture namespace already exists'
out=app/'build/terminal-validation';out.mkdir(parents=True,exist_ok=True)
scenarios=[(f'case-{n}',str(n).encode(),(app/f'data/expected-{n}.txt').read_bytes(),0) for n in range(8)]
scenarios += [(name,data,b'invalid command: expected one byte 0..7 or EOF\n',64) for name,data in [('bad-letter',b'x'),('bad-digit',b'8'),('bad-newline',b'\n')]]
scenarios += [('eof',b'',(app/'data/expected-0.txt').read_bytes(),0)]
manifest={};sources=['main.omg','domain/model.omg','domain/search.omg','data/fixtures.omg']
try:
 for name,data,expected,exit_code in scenarios:
  case=fixture_parent/name;case.mkdir(parents=True)
  for relative in sources:
   destination=case/relative;destination.parent.mkdir(parents=True,exist_ok=True)
   payload=(app/relative).read_bytes();destination.write_bytes(payload)
   assert destination.read_bytes()==payload
   manifest[f'{name}/{relative}']=hashlib.sha256(payload).hexdigest()
  # Only the navigation spelling changes; the canonical std source is identical.
  build=(app/'build.omg').read_text().replace('../../../../source/library/std',os.path.relpath(repo/'source/library/std',case))
  (case/'build.omg').write_text(build);(case/'input.txt').write_bytes(data)
  (case/'expected_stdout.txt').write_bytes(expected);(case/'expected_native_exit.txt').write_text(str(exit_code)+'\n')
  for relative in ['build.omg','input.txt','expected_stdout.txt','expected_native_exit.txt']:
   manifest[f'{name}/{relative}']=hashlib.sha256((case/relative).read_bytes()).hexdigest()
 # Freeze all complete sources, runtime inputs and independently authored oracles
 # before the runner starts. Keep exact fixtures as reviewable supporting artifacts.
 (out/'pre-execution-hashes.json').write_text(json.dumps(manifest,indent=2)+'\n')
 shutil.copytree(fixture_parent,out/'fixtures',dirs_exist_ok=True)
 command=[sys.executable,str(repo/'tools/corpus_gate.py'),'--runner',str(pathlib.Path(a.runner).resolve()),'--both','--filter','run/calls/new070_sensor_stdout_validation/','--json',str(out/'verdict.json')]
 env=dict(os.environ,OMEGA_CORPUS_JOBS='1',OMEGA_CORPUS_FIXTURE_SECS='1200',OMEGA_LIBRARY_ROOT=str(repo/'source/library'))
 (out/'command.json').write_text(json.dumps({'command':command,'jobs':1,'target':'linux_x86_64','numeric_tolerance':0,'terminal_stdout':'authored corpus stdout oracle (documented line-ending normalization)'},indent=2)+'\n')
 r=subprocess.run(command,cwd=repo,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 (out/'gate.stdout').write_bytes(r.stdout);(out/'gate.stderr').write_bytes(r.stderr)
 d=json.loads((out/'verdict.json').read_text());assert r.returncode in (0,1),r.stderr.decode()
 assert not d.get('infrastructure_error') and not d.get('oracle_failures'),d
 observations=d['observations'];prefix='run/calls/new070_sensor_stdout_validation/'
 assert {o['fixture'] for o in observations}=={prefix+s[0] for s in scenarios},d
 expected_exits={prefix+name:str(exit_code) for name,_,_,exit_code in scenarios}
 for o in observations:
  f=o['facts'];exit_code=expected_exits[o['fixture']]
  assert o['status']=='built' and not o['diagnostics'],o
  assert f.get('exit')==exit_code and f.get('interpreter-exit')==exit_code,o
  assert f.get('stdout')=='match' and f.get('interpreter-stdout')=='match' and f.get('interpreter')=='agrees',o
 print('PASS: all 12 native and admitted Terminal stdout/exit oracles; no declines or unchecked routes')
finally:
 if fixture_parent.exists():shutil.rmtree(fixture_parent)
