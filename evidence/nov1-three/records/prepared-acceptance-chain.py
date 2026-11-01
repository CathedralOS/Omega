import hashlib,importlib.util,json,os,shutil,subprocess,sys,time
from pathlib import Path
root=Path.cwd();out=root/'build/swarm/nov1';generation=next((out/'producers').glob('gen-ea132c1fff8*'));cli=str(generation/'omega');env=dict(os.environ,CARGO_PROFILE_DEV_OPT_LEVEL='1',RAYON_NUM_THREADS='2',OMEGA_CORPUS_JOBS='2');os.environ.update({k:env[k] for k in ['CARGO_PROFILE_DEV_OPT_LEVEL','RAYON_NUM_THREADS','OMEGA_CORPUS_JOBS']})
def run(name,command,allowed=(0,),timeout=1800):
 started=time.monotonic();r=subprocess.run(command,stdin=subprocess.DEVNULL,capture_output=True,text=True,env=env,timeout=timeout);(out/(name+'.stdout')).write_text(r.stdout);(out/(name+'.stderr')).write_text(r.stderr);print(name,'exit',r.returncode,'seconds',round(time.monotonic()-started,2),flush=True)
 if r.returncode not in allowed:print((r.stdout+r.stderr)[-9500:],flush=True);raise SystemExit(r.returncode or 1)
 return r
revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip();manifest=json.loads((generation/'GENERATION').read_text())
for path,identity in manifest['source_inputs'].items():
 if subprocess.check_output(['git','rev-parse','HEAD:'+path],text=True).strip()!=identity:raise SystemExit('producer input changed: '+path)
facts=[('open_const_implementation',13,16),('open_record_const_implementation',10,10)]
sys.path.insert(0,str(root/'tools'));import sample_native_gate as gate
for project in ['samples/cli/facts/open_const_implementation/const_name/statement_result','samples/cli/facts/open_const_implementation/const_name','samples/cli/facts/open_const_implementation','samples/cli/facts/open_record_const_implementation']:
 started=time.monotonic();reason,output=gate.accept_review(cli,project,'linux_x86_64',180);identity=project.replace('/','_');(out/('refresh-'+identity+'.txt')).write_text(output);print('refresh',project,'reason',reason,'seconds',round(time.monotonic()-started,2),flush=True)
 if reason:print(output[-6000:]);raise SystemExit(1)
config=json.loads((out/'final-qualification-config.json').read_text())
for check in config['checks']:
 command=check['command'];position=command.index('--output')+1;command[position]+='-prepared'
(out/'prepared-qualification-config.json').write_text(json.dumps(config,indent=2)+'\n');state=out/'cleanup-baseline.json'
def state_revision():return str(json.loads(state.read_text())['revision'])
run('prepared-qualification-amend',['python3','tools/qualification.py','amend','--state',str(state),'--revision',state_revision(),'--config',str(out/'prepared-qualification-config.json'),'--reason','Refresh ordinary nested package review child-before-parent after retained setup refusal. Sources and immutable semantic producer are unchanged. Preserve actual ten engine agreements and sixteen intended negatives from the initial capture; fresh complete acceptance uses settled exact locks.'])
run('prepared-qualification-assess',['python3','tools/qualification.py','assess','--state',str(state),'--revision',state_revision(),'--base','bfcffe6bcfcedb6fd60713e223408566c6c4e10d'])
for name,_,_ in facts:run('prepared-qualified-'+name,['python3','tools/qualification.py','run','--state',str(state),'--revision',state_revision(),'--check',name])
filters='facts/open_const_implementation_,facts/open_record_const_implementation_,generics/open_const_,modules/open_const_private_implementation_projection_exit,traits/public_private_post_write_guarantee,traits/public_guarantee_,borrow/write_only_observation,borrow/write_only_state_readable_widening,traits/captured_const_contract_distinct_binders,traits/requirement_application_guarantee_identity,providers/primitive_reference_contract_refinement'
run('latest-source-compare',['python3','tools/corpus_gate.py','--runner',str(generation/'corpus_runner'),'--baseline-runner','/home/jarod/development/Omega/.agents/worktree/nov1-coord/build/swarm/nov1/producers/gen-bfcffe6bcfc/corpus_runner','--compare','bfcffe6bcfcedb6fd60713e223408566c6c4e10d','--jev','--filter',filters,'--write-moves',str(out/'latest-source-moves.json'),'--json',str(out/'latest-source-compare.json')],allowed=(0,1,3))
run('latest-corpus-both',['python3','tools/corpus_gate.py','--runner',str(generation/'corpus_runner'),'--baseline-runner','/home/jarod/development/Omega/.agents/worktree/nov1-coord/build/swarm/nov1/producers/gen-bfcffe6bcfc/corpus_runner','--compare','bfcffe6bcfcedb6fd60713e223408566c6c4e10d','--both','--filter',filters,'--jev','--write-moves',str(out/'latest-both-moves.json'),'--json',str(out/'latest-corpus-both.json')],allowed=(0,1,3))
print('focused chain complete; raw records retained',flush=True)
