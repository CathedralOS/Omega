from pathlib import Path
import json,subprocess,shutil,hashlib,sys,os
w=Path('build/proof-profile/lazy-reference-graph');messages=(w/'corpus-build.jsonl').read_text();artifacts=[]
for line in messages.splitlines():
 try:x=json.loads(line)
 except ValueError:continue
 if x.get('reason')=='compiler-artifact' and x.get('target',{}).get('name')=='corpus_runner' and x.get('executable'):artifacts.append(Path(x['executable']))
assert len(artifacts)==1
sys.path.insert(0,'tools');from compiler_profile import require_optimized,record_cargo_build
record_cargo_build(artifacts[0],messages)
profile=require_optimized(artifacts[0]);dest=Path('build/proof-profile/producers/lazy-reference/corpus_runner.exe');shutil.copy2(artifacts[0],dest)
profile_frozen=json.loads(subprocess.check_output([str(dest.resolve()),'--build-profile']));assert int(profile_frozen['opt_level'])>0
(w/'corpus-identity.json').write_text(json.dumps(dict(executable=str(dest.resolve()),sha256=hashlib.sha256(dest.read_bytes()).hexdigest(),build_profile=profile_frozen,build_artifact=str(artifacts[0])),indent=2)+'\n',encoding='utf-8',newline='\n')
env=os.environ.copy();env['OMEGA_CORPUS_JOBS']='1'
for k in list(env):
 if k.startswith('OMEGA_') and any(x in k for x in ['PROBE','PROFILE','COUNTS','MEASUREMENT']):env.pop(k,None)
positive='pass/borrows/returned_reference_source_guarantee_exit,pass/borrows/completed_reference_result_preserves_input_holds_exit,pass/borrows/runtime_selected_shared_reference_cohort_exit,pass/calls/runtime_cyclic_state_reference_result_exit'
negative='fail/borrows/returned_reference_missing_membership,fail/borrows/returned_reference_invalidated_membership,fail/borrows/dropped_reference_qualification_mixed_origins'
for label,flags,selection in [('positive',['--both'],positive),('negative',[],negative)]:
 command=['python','tools/corpus_gate.py','--runner',str(dest.resolve()),*flags,'--filter',selection,'--json',str((w/('quality-'+label+'.json')).resolve())]
 r=subprocess.run(command,env=env,capture_output=True,text=True,encoding='utf-8');(w/('quality-'+label+'.log')).write_text(r.stdout+r.stderr,encoding='utf-8',newline='\n');print(label,r.returncode,(r.stdout+r.stderr)[-1600:],flush=True)
 if r.returncode:raise SystemExit(r.returncode)
