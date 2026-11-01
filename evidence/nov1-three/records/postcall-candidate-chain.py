import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
root=Path.cwd();out=root/'build/swarm/nov1';env=dict(os.environ,CARGO_PROFILE_DEV_OPT_LEVEL='1',RAYON_NUM_THREADS='2',OMEGA_CORPUS_JOBS='2');revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip();generation=out/('producers/gen-'+revision[:12]);cli=str(generation/'omega')
def run(name,args):
 start=time.monotonic();r=subprocess.run(args,capture_output=True,text=True,env=env);(out/(name+'.stdout')).write_text(r.stdout);(out/(name+'.stderr')).write_text(r.stderr);print(name,r.returncode,round(time.monotonic()-start,2),flush=True)
 if r.returncode:print((r.stdout+r.stderr)[-8000:],flush=True);raise SystemExit(r.returncode)
run('postcall-affected-check',['mbx','check','-p','validation','-p','typed-trees-to-checked-trees','-p','checked-interpreter','-p','typed-trees'])
run('postcall-cli-build',['mbx','build','-p','omega','--message-format=json'])
generation.mkdir(parents=True,exist_ok=False);shutil.copy2(root/'target/debug/omega',generation/'omega');(generation/'profile.json').write_text(subprocess.check_output([cli,'--build-profile'],text=True));manifest={'revision':revision,'base':'bfcffe6bcfcedb6fd60713e223408566c6c4e10d','command':['mbx','build','-p','omega','--message-format=json'],'original_artifact':str(root/'target/debug/omega'),'sha256':hashlib.sha256((generation/'omega').read_bytes()).hexdigest(),'source_inputs':{p:subprocess.check_output(['git','rev-parse','HEAD:'+p],text=True).strip() for p in ['omega-rust','source/library','Cargo.toml','Cargo.lock','rust-toolchain.toml']}};(generation/'GENERATION').write_text(json.dumps(manifest,indent=2)+'\n');(generation/'READY').write_text('immutable optimized CLI\n')
run('postcall-statement-check',[cli,'--check','samples/cli/facts/open_const_implementation/const_name/statement_result/main.omg'])
print('Frozen statement check passed; prepare ordinary package runtime next.',flush=True)
