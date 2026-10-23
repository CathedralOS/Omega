import os,subprocess,sys
from pathlib import Path
sys.path.insert(0,str(Path('tools').resolve()))
import build_slot
build_slot.POLL_SECONDS=.05
out=Path('build/deferred-completion-storage');name=sys.argv[1]
os.environ['CARGO_TARGET_DIR']=str(Path('build/gate-cli').resolve());os.environ['CARGO_BUILD_JOBS']='2'
with build_slot.hold():
 with (out/(name+'-build.jsonl')).open('w') as stdout,(out/(name+'-build.log')).open('w') as stderr:
  result=subprocess.run(['mbx','build','-j','2','-p','omega','--bin','omega','--message-format=json-render-diagnostics'],stdout=stdout,stderr=stderr)
 assert result.returncode==0,result.returncode
subprocess.run(['python3',str(out/'freeze.py'),name],check=True)
