import hashlib,json,os,shutil,subprocess,sys,time
from pathlib import Path
root=Path.cwd();out=root/'build/swarm/nov1';revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip();generation=out/'producers'/('gen-'+revision[:12]);env=dict(os.environ,CARGO_PROFILE_DEV_OPT_LEVEL='1',RAYON_NUM_THREADS='2',OMEGA_CORPUS_JOBS='2')
def run(name,command,allowed=(0,),timeout=1800):
 started=time.monotonic();r=subprocess.run(command,stdin=subprocess.DEVNULL,capture_output=True,text=True,env=env,timeout=timeout)
 (out/(name+'.stdout')).write_text(r.stdout);(out/(name+'.stderr')).write_text(r.stderr)
 print(name,'exit',r.returncode,'elapsed',round(time.monotonic()-started,2),flush=True)
 if r.returncode not in allowed:
  print((r.stdout+r.stderr)[-9000:],flush=True);sys.exit(r.returncode or 1)
 return r
r=run('second-candidate-build',['mbx','build','-p','omega','--message-format=json']);generation.mkdir(parents=True,exist_ok=False);shutil.copy2(root/'target/debug/omega',generation/'omega')
profile=subprocess.check_output([str(generation/'omega'),'--build-profile'],text=True);(generation/'profile.json').write_text(profile)
manifest={'revision':revision,'base':'bfcffe6bcfcedb6fd60713e223408566c6c4e10d','command':['mbx','build','-p','omega','--message-format=json'],'original_artifact':str(root/'target/debug/omega'),'sha256':hashlib.sha256((generation/'omega').read_bytes()).hexdigest(),'profile':json.loads(profile),'source_inputs':{p:subprocess.check_output(['git','rev-parse',revision+':'+p],text=True).strip() for p in ['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']}}
(generation/'GENERATION').write_text(json.dumps(manifest,indent=2)+'\n');(generation/'READY').write_text('immutable optimized CLI\n')
run('second-original-row',['python3','tools/sample_native_gate.py','--cli',str(generation/'omega'),'--facts','--filter','cli/facts/open_const_implementation','--jobs','2','--write-results',str(out/'second-original-row.json')])
for name,source in [('scalar-package','build/swarm/nov1/probes/open_const_package_scalar/main.omg'),('record-package','build/swarm/nov1/probes/open_const_package_record/main.omg')]:
 run('second-'+name,['python3','build/swarm/nov1/openconst-baseline-validator.py','--cli',str(generation/'omega'),'--source',str(root/source)])
run('second-affected-check',['mbx','check','-p','typed-trees-to-checked-trees','-p','checked-interpreter','-p','typed-trees'])
run('second-affected-clippy',['mbx','clippy','-p','typed-trees-to-checked-trees','-p','checked-interpreter','-p','typed-trees','--no-deps','--','-D','warnings'])
run('second-runner-build',['mbx','test','--no-run','--message-format=json','-p','compiler','--test','corpus_runner'])
runner=None
for line in (out/'second-runner-build.stdout').read_text().splitlines():
 try:message=json.loads(line)
 except ValueError:continue
 if message.get('reason')=='compiler-artifact' and message.get('target',{}).get('name')=='corpus_runner':runner=message.get('executable') or runner
if not runner:raise SystemExit('runner artifact absent')
shutil.copy2(runner,generation/'corpus_runner');(generation/'RUNNER_GENERATION').write_text(json.dumps({'revision':revision,'command':['mbx','test','--no-run','--message-format=json','-p','compiler','--test','corpus_runner'],'original_artifact':runner,'sha256':hashlib.sha256((generation/'corpus_runner').read_bytes()).hexdigest()},indent=2)+'\n')
print('candidate immutable generation',generation,flush=True)
