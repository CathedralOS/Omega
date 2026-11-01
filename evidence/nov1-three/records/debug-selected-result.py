import os,subprocess,sys,time
from pathlib import Path
root=Path('build/swarm/nov1');started=time.monotonic()
env=dict(os.environ,CARGO_PROFILE_DEV_OPT_LEVEL='1')
r=subprocess.run(['mbx','build','-p','omega','--message-format=json'],stdin=subprocess.DEVNULL,capture_output=True,text=True,env=env)
(root/'debug-build.jsonl').write_text(r.stdout);(root/'debug-build.stderr').write_text(r.stderr)
print('debug build exit',r.returncode,'elapsed',round(time.monotonic()-started,2),flush=True)
if r.returncode:print(r.stderr[-7000:]);sys.exit(r.returncode)
env['OMEGA_DEBUG_SELECTED_RESULT']='selected'
r=subprocess.run(['target/debug/omega','--check','samples/cli/facts/open_const_implementation/local/main.omg'],stdin=subprocess.DEVNULL,capture_output=True,text=True,env=env,timeout=60)
(root/'debug-selected.stdout').write_text(r.stdout);(root/'debug-selected.stderr').write_text(r.stderr)
print(r.stdout+r.stderr,flush=True);print('source check exit',r.returncode,flush=True)
