from pathlib import Path
import subprocess,json,shutil,hashlib,sys,os
w=Path('build/proof-profile/lazy-reference-graph');messages=(w/'baseline-corpus-build.jsonl').read_text();artifacts=[]
for line in messages.splitlines():
 try:x=json.loads(line)
 except ValueError:continue
 if x.get('reason')=='compiler-artifact' and x.get('target',{}).get('name')=='corpus_runner' and x.get('executable'):artifacts.append(Path(x['executable']))
assert len(artifacts)==1
sys.path.insert(0,'tools');from compiler_profile import require_optimized,record_cargo_build
record_cargo_build(artifacts[0],messages);require_optimized(artifacts[0]);dest=Path('build/proof-profile/producers/baseline/corpus_runner.exe');shutil.copy2(artifacts[0],dest)
profile=json.loads(subprocess.check_output([str(dest.resolve()),'--build-profile']));assert int(profile['opt_level'])>0
(w/'baseline-corpus-identity.json').write_text(json.dumps(dict(executable=str(dest.resolve()),sha256=hashlib.sha256(dest.read_bytes()).hexdigest(),build_profile=profile,build_artifact=str(artifacts[0])),indent=2)+'\n',encoding='utf-8',newline='\n')
env=os.environ.copy();env['OMEGA_CORPUS_JOBS']='1'
for k in list(env):
 if k.startswith('OMEGA_') and any(x in k for x in ['PROBE','PROFILE','COUNTS','MEASUREMENT']):env.pop(k,None)
summary={}
for label,flags in [('positive',['--both']),('negative',[])]:
 candidate=json.loads((w/('quality-'+label+'.json')).read_text());command=['python','tools/corpus_gate.py','--runner',str(dest.resolve()),*flags,'--filter',candidate['filter'],'--json',str((w/('baseline-quality-'+label+'.json')).resolve())]
 r=subprocess.run(command,env=env,capture_output=True,text=True,encoding='utf-8');(w/('baseline-quality-'+label+'.log')).write_text(r.stdout+r.stderr,encoding='utf-8',newline='\n')
 baseline=json.loads((w/('baseline-quality-'+label+'.json')).read_text());assert not baseline['oracle_failures'] and not baseline['infrastructure_error']
 def observed(x):return [{k:o[k] for k in ['fixture','tier','status','expected_satisfied','facts','diagnostics']} for o in x['observations']]
 assert observed(baseline)==observed(candidate),(label,observed(baseline),observed(candidate))
 summary[label]=dict(candidate_exit=candidate['exit_code'],baseline_exit=r.returncode,observations=observed(candidate),candidate_golden_moves=candidate['fixtures'],baseline_golden_moves=baseline['fixtures'])
 print(label,'baseline/candidate observations identical; gate exits',r.returncode,candidate['exit_code'],flush=True)
(w/'quality-summary.json').write_text(json.dumps(dict(positive_oracles=4,negative_rejects=3,baseline_candidate_equal=True,golden_recorded=False,legs=summary),indent=2)+'\n',encoding='utf-8',newline='\n')
i=json.loads((w/'candidate-source.json').read_text());saved=(w/'pure-candidate.rs').read_bytes();assert hashlib.sha256(saved).hexdigest()==i['source_sha256'];Path(i['file']).write_bytes(saved)
print('Restored exact pure candidate after baseline control')
