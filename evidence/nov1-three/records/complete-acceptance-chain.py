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
run('validation-affected-clippy',['mbx','clippy','-p','validation','--no-deps','--message-format=json','--','-D','warnings'],allowed=(0,101))
run('latest-affected-clippy',['mbx','clippy','-p','typed-trees-to-checked-trees','-p','checked-interpreter','--no-deps','--message-format=json','--','-D','warnings'],allowed=(0,101))
r=run('latest-runner-build',['mbx','test','--no-run','--message-format=json','-p','compiler','--test','corpus_runner']);runner=None
for line in r.stdout.splitlines():
 try:message=json.loads(line)
 except ValueError:continue
 if message.get('reason')=='compiler-artifact' and message.get('target',{}).get('name')=='corpus_runner':runner=message.get('executable') or runner
if not runner:raise SystemExit('runner artifact absent')
shutil.copy2(runner,generation/'corpus_runner');(generation/'RUNNER_GENERATION').write_text(json.dumps({'actual_build_revision':revision,'compiler_source_revision':manifest['revision'],'source_inputs':manifest['source_inputs'],'command':['mbx','test','--no-run','--message-format=json','-p','compiler','--test','corpus_runner'],'original_artifact':runner,'sha256':hashlib.sha256((generation/'corpus_runner').read_bytes()).hexdigest()},indent=2)+'\n')
(generation/'runner-profile.json').write_text(subprocess.check_output([str(generation/'corpus_runner'),'--build-profile'],text=True))
sys.path.insert(0,str(root/'tools'));import sample_native_gate as gate
spec=importlib.util.spec_from_file_location('runtime_row',out/'runtime-row.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
facts=[('open_const_implementation',13,16),('open_record_const_implementation',10,10)];locks=[]
for name,positive_count,negative_count in facts:
 positives,negatives=module.projects(Path('samples/cli/facts')/name)
 if len(positives)!=positive_count or len(negatives)!=negative_count:raise SystemExit('unexpected source count '+name+str((len(positives),len(negatives))))
 for source in positives:
  started=time.monotonic();reason,output=gate.accept_review(cli,str(source.parent),'linux_x86_64',180);identity=name+'_'+source.parent.relative_to(Path('samples/cli/facts')/name).as_posix().replace('/','_');(out/('prepare-'+identity+'.txt')).write_text(output);print('prepare',source.parent,'reason',reason,'seconds',round(time.monotonic()-started,2),flush=True)
  if reason:print(output[-6000:],flush=True);raise SystemExit(1)
  locks.append(str((source.parent/'omega.lock').resolve()))
config=json.loads((out/'statement-postcall-baseline-config.json').read_text());template=dict(config['checks'][0]);config['checks']=[]
for name,positive_count,negative_count in facts:
 check=dict(template);check.update(id=name,command=['python3',str(out/'runtime-row.py'),'--cli',cli,'--fact','samples/cli/facts/'+name,'--output',str(out/('qualified-'+name)),'--positives',str(positive_count),'--negatives',str(negative_count)],inputs=sorted(set(template['inputs']+['tools','wiki/spec/language/evaluation.md','wiki/spec/language/machines.md','wiki/spec/language/state_contracts.md'])),artifacts=[cli,str(generation/'GENERATION'),str(generation/'profile.json'),str(out/'final-postcall-cli-build.stdout'),str(out/'final-postcall-cli-build.stderr'),str(out/'runtime-row.py'),*locks],producer=dict(template['producer'],revision=manifest['revision'],executables=[cli]),final=True,timeout_seconds=900,closure_reason='Exact selected public contract and complete applied execution retain const binder and package closure across the existing row. Original source passed before runtime-only oracle strengthening. All positive and intended-invalid sources, full compiler/library/toolchain/tooling closure, immutable producer/build evidence and ordinary accepted locks are bound. Every native and Terminal exit plus intended refusal fragment is observed explicitly.')
 config['checks'].append(check)
(out/'final-qualification-config.json').write_text(json.dumps(config,indent=2)+'\n')
state=out/'cleanup-baseline.json'
def state_revision():return str(json.loads(state.read_text())['revision'])
run('final-qualification-amend',['python3','tools/qualification.py','amend','--state',str(state),'--revision',state_revision(),'--config',str(out/'final-qualification-config.json'),'--reason','Adopt final same-fact runtime/negative acceptance after ordinary package setup; retain all measured failures, original successes and immutable producers in history. The current immutable producer is built from exact clean ea132c1fff8 semantic source; all prior refusals and original-source passes remain historical.'])
run('final-qualification-assess',['python3','tools/qualification.py','assess','--state',str(state),'--revision',state_revision(),'--base','bfcffe6bcfcedb6fd60713e223408566c6c4e10d'])
for name,_,_ in facts:run('qualified-'+name,['python3','tools/qualification.py','run','--state',str(state),'--revision',state_revision(),'--check',name])
filters='facts/open_const_implementation_,facts/open_record_const_implementation_,generics/open_const_,modules/open_const_private_implementation_projection_exit,traits/public_private_post_write_guarantee,traits/public_guarantee_,borrow/write_only_observation,borrow/write_only_state_readable_widening,traits/captured_const_contract_distinct_binders,traits/requirement_application_guarantee_identity,providers/primitive_reference_contract_refinement'
run('latest-source-compare',['python3','tools/corpus_gate.py','--runner',str(generation/'corpus_runner'),'--baseline-runner','/home/jarod/development/Omega/.agents/worktree/nov1-coord/build/swarm/nov1/producers/gen-bfcffe6bcfc/corpus_runner','--compare','bfcffe6bcfcedb6fd60713e223408566c6c4e10d','--jev','--filter',filters,'--write-moves',str(out/'latest-source-moves.json'),'--json',str(out/'latest-source-compare.json')],allowed=(0,1,3))
run('latest-corpus-both',['python3','tools/corpus_gate.py','--runner',str(generation/'corpus_runner'),'--baseline-runner','/home/jarod/development/Omega/.agents/worktree/nov1-coord/build/swarm/nov1/producers/gen-bfcffe6bcfc/corpus_runner','--compare','bfcffe6bcfcedb6fd60713e223408566c6c4e10d','--both','--filter',filters,'--jev','--write-moves',str(out/'latest-both-moves.json'),'--json',str(out/'latest-corpus-both.json')],allowed=(0,1,3))
print('focused chain complete; raw records retained',flush=True)
