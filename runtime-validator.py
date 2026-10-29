from pathlib import Path
import json,subprocess,os,sys,time
r=Path('build/retired-state-consumers');scope=json.loads((r/'closure-validation-scope.json').read_text());runner=str((r/'closure-producer/corpus_runner').resolve());env=dict(os.environ,OMEGA_CORPUS_JOBS='2')
for leg,args in [('both',['--both'])]:
 if len(sys.argv)>1 and sys.argv[1]!=leg:continue
 command=['python3','tools/corpus_gate.py','--runner',runner,*args,'--filter',','.join(scope['source' if leg=='source' else 'runtime']),'--json',str(r/('closure-corrected-'+leg+'.json'))];start=time.monotonic()
 with(r/('closure-corrected-'+leg+'.stdout')).open('w') as out,(r/('closure-corrected-'+leg+'.stderr')).open('w') as err:result=subprocess.run(command,stdin=subprocess.DEVNULL,stdout=out,stderr=err,env=env,timeout=900)
 receipt=dict(command=command,exit=result.returncode,elapsed_seconds=time.monotonic()-start);(r/('closure-corrected-'+leg+'-command.json')).write_text(json.dumps(receipt,indent=2)+'\n');print(leg,result.returncode,round(receipt['elapsed_seconds'],2),flush=True)
 data=json.loads((r/('closure-corrected-'+leg+'.json')).read_text());assert data['infrastructure_error'] is None,data;actual={a['fixture']:a for a in data['observations']};assert set(scope['source' if leg=='source' else 'runtime'])<=actual.keys()
 bad=[]
 inherited_observed=[]
 inherited_diagnostics={'pass/slices/runtime_nested_view_descriptor_reseat_exit':'exactly one parent entry constraint','pass/borrows/runtime_released_loan_with_forwarded_view_exit':'SourceCustodyMismatch { machine: Some(MachineId(30)), operation: Some(OperationId(216))'}
 for f,a in actual.items():
  if f in inherited_diagnostics and a['status']=='rejected' and any(inherited_diagnostics[f] in d for d in a['diagnostics']):
   inherited_observed.append(f)
  elif f.startswith('fail/'):
   if a['status']!='rejected' or a['expected_satisfied'] is not True:bad.append(a)
  elif leg=='source':
   if a['status']!='checked':bad.append(a)
  elif a['status']!='built':bad.append(a)
  elif a['facts'].get('interpreter')!='agrees':
   # Deliberate counterfactual programs author a native trap. Both engines
   # trapping fulfils that oracle; it is not a successful normal execution.
   expected=Path('tests/omega')/f/'expected_native_exit.txt'
   if not(expected.is_file() and expected.read_text().strip()=='trap' and a['facts'].get('interpreter')=='trapped' and a['facts'].get('exit')=='signal'):bad.append(a)
 assert not bad,bad
 # The separately frozen baseline proves only this unchanged inherited refusal.
 failures=[f for f in data['oracle_failures'] if not any(x in str(f) for x in inherited_observed)]
 assert not failures,failures
 print('validated',leg,len(actual),'inherited observations',inherited_observed,flush=True)
