from pathlib import Path
import subprocess,json,os
w=Path('build/proof-profile/lazy-reference-graph');positive=json.loads((w/'quality-positive.json').read_text())
assert positive['counts']['fixtures']==4 and not positive['oracle_failures'] and not positive['infrastructure_error']
assert all(o['status']=='built' and o['facts'].get('exit')=='0' and o['facts'].get('interpreter-exit')=='0' and o['facts'].get('interpreter')=='agrees' for o in positive['observations'])
assert all(f['verdict']=='new' and f['before'] is None for f in positive['fixtures'])
env=os.environ.copy();env['OMEGA_CORPUS_JOBS']='1'
for k in list(env):
 if k.startswith('OMEGA_') and any(x in k for x in ['PROBE','PROFILE','COUNTS','MEASUREMENT']):env.pop(k,None)
selection='fail/borrows/returned_reference_missing_membership,fail/borrows/returned_reference_invalidated_membership,fail/borrows/dropped_reference_qualification_mixed_origins'
command=['python','tools/corpus_gate.py','--runner',str(Path('build/proof-profile/producers/lazy-reference/corpus_runner.exe').resolve()),'--filter',selection,'--json',str((w/'quality-negative.json').resolve())]
r=subprocess.run(command,env=env,capture_output=True,text=True,encoding='utf-8');(w/'quality-negative.log').write_text(r.stdout+r.stderr,encoding='utf-8',newline='\n');print(r.returncode,(r.stdout+r.stderr)[-1800:],flush=True)
x=json.loads((w/'quality-negative.json').read_text());assert x['counts']['fixtures']==3 and not x['oracle_failures'] and not x['infrastructure_error']
assert all(o['status']=='rejected' and o['expected_satisfied'] for o in x['observations'])
if r.returncode:assert r.returncode==1 and all(f['verdict']=='new' and f['before'] is None for f in x['fixtures'])
(w/'quality-summary.json').write_text(json.dumps(dict(positive_oracles=4,negative_rejects=3,positive_gate_exit=positive['exit_code'],negative_gate_exit=r.returncode,positive_golden_moves=positive['fixtures'],negative_golden_moves=x['fixtures'],golden_recorded=False),indent=2)+'\n',encoding='utf-8',newline='\n')
print('Authored oracles:4 both-engine positives;3 expected source rejections. Golden state preserved.')
