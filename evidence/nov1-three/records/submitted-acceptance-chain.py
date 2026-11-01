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
# Nested project sources and compiler inputs are unchanged; refreshing the two
# README-owning parent reviews is outside the frozen final capture.
for project in ['samples/cli/facts/open_const_implementation','samples/cli/facts/open_record_const_implementation']:
 reason,output=gate.accept_review(cli,project,'linux_x86_64',180);(out/('submitted-prepare-'+Path(project).name+'.txt')).write_text(output);print('submitted prepare',project,reason,flush=True)
 if reason:print(output[-6000:]);raise SystemExit(1)
state=out/'cleanup-baseline.json'
def state_revision():return str(json.loads(state.read_text())['revision'])
run('submitted-qualification-amend',['python3','tools/qualification.py','amend','--state',str(state),'--revision',state_revision(),'--config',str(out/'submitted-qualification-config.json'),'--reason','Final source contains accepted compiler repair, actual scoped pins and requirements/evidence README. Preserve prior observations; retain final:true and freshly capture the exact submitted source without a compiler rebuild.'])
run('submitted-qualification-assess',['python3','tools/qualification.py','assess','--state',str(state),'--revision',state_revision(),'--base','bfcffe6bcfcedb6fd60713e223408566c6c4e10d'])
for name,_,_ in facts:run('submitted-qualified-'+name,['python3','tools/qualification.py','run','--state',str(state),'--revision',state_revision(),'--check',name])
run('submitted-independent-review-context',['python3','tools/qualification.py','handoff','--state',str(state),'--revision',state_revision(),'--role','review','--previous',str(out/'current-independent-review-packet.json')])
print('Final candidate qualification complete; no rebuild or main movement.',flush=True)
