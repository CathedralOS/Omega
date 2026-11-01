from pathlib import Path
import hashlib,json,os,subprocess,sys
p=Path('/Users/zcanann/Documents/Projects/Omega/build/window-demo-qualification-20261101');producer='case-bound-applied-r3';records=[]
places=['local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body']
for place in places:
 root=Path('samples/cli/facts/case_dependent_default_domain')/place
 inputs={str(f):hashlib.sha256(f.read_bytes()).hexdigest() for f in [root/'main.omg',root/'build.omg',root/'invalid/main.omg']}
 for stage,arguments in [('update',['update','--project',str(root),'--target','macos_arm64','--offline']),('native',['run','--both',str(root/'main.omg')]),('reject',['--check',str(root/'invalid/main.omg')])]:
  label='case-row-r3-'+place+'-'+stage
  env=dict(os.environ,VALIDATION_PROJECT=str(root/'invalid' if stage=='reject' else root),VALIDATION_PRODUCER=producer)
  subprocess.run([sys.executable,str(p/'run_cli.py'),label,*arguments],env=env,stdout=subprocess.DEVNULL,check=True)
  result=json.loads((p/(label+'.json')).read_text());text=Path(result['stdout']).read_text()+Path(result['stderr']).read_text()
  expected=result['exit']==0 if stage=='update' else (result['exit']==0 and 'native exit 0' in text and 'interp exit: 0' in text and 'DECLINED' not in text if stage=='native' else result['exit']==1 and 'cannot prove requires contract for call width_range' in text)
  row={'place':place,'stage':stage,'exit':result['exit'],'seconds':result['seconds'],'expected':expected,'inputs':inputs,'record':str(p/(label+'.json'))};records.append(row)
  (p/'case-row-r3-results.json').write_text(json.dumps(records,indent=2)+'\n')
  print(json.dumps({k:row[k] for k in ['place','stage','exit','seconds','expected']}),flush=True)
  assert all(hashlib.sha256(Path(f).read_bytes()).hexdigest()==h for f,h in inputs.items())
  assert expected,text[-4500:]
