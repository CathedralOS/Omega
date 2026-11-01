import os,subprocess,time
from pathlib import Path
out=Path('build/swarm/nov1');env=dict(os.environ,CARGO_PROFILE_DEV_OPT_LEVEL='1',RAYON_NUM_THREADS='2')
def run(name,args):
 start=time.monotonic();r=subprocess.run(args,capture_output=True,text=True,env=env);(out/(name+'.stdout')).write_text(r.stdout);(out/(name+'.stderr')).write_text(r.stderr);print(name,r.returncode,round(time.monotonic()-start,2),flush=True);return r.returncode
if run('postcall-diagnostic-build',['mbx','build','-p','omega','--message-format=json']):raise SystemExit(1)
env['OMEGA_NOV1_POSTCALL_TRACE']='1'
run('postcall-diagnostic-check',['target/debug/omega','--check','samples/cli/facts/open_const_implementation/const_name/statement_result/main.omg'])
