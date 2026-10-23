import os, sys, subprocess
from pathlib import Path
sys.path.insert(0,str(Path('tools').resolve()))
import build_slot
build_slot.POLL_SECONDS=.05
folder=Path('build/placed-result-selection')
os.environ['CARGO_TARGET_DIR']=str(Path('build/gate-cli').resolve())
os.environ['CARGO_BUILD_JOBS']='2'
with build_slot.hold():
 with (folder/'instrumented-build.jsonl').open('w') as out, (folder/'instrumented-build.log').open('w') as err:
  result=subprocess.run(['mbx','build','-j','2','-p','omega','--bin','omega','--message-format=json-render-diagnostics'],stdout=out,stderr=err)
 print('build exit',result.returncode,flush=True)
 sys.exit(result.returncode)
