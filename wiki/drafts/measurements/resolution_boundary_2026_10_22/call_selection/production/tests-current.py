import os,subprocess,sys,time,json
from pathlib import Path
sys.path.insert(0,str(Path('tools').resolve()))
import build_slot
build_slot.POLL_SECONDS=.05
out=Path('build/placed-result-selection-production')
os.environ['CARGO_TARGET_DIR']=str(Path('build/gate-cli').resolve());os.environ['CARGO_BUILD_JOBS']='2'
start=time.monotonic()
while not (out/'candidate-producer.json').exists():
 if time.monotonic()-start>3600:raise TimeoutError('production build incomplete')
 time.sleep(1)
with build_slot.hold():
 with (out/'tests-current.log').open('w') as log:
  result=subprocess.run(['mbx','nextest','run','-p','syntax-trees-to-symbol-resolved-trees'],stdout=log,stderr=subprocess.STDOUT)
record=dict(revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),exit=result.returncode)
(out/'tests-current.json').write_text(json.dumps(record,indent=2)+'\n')
assert result.returncode==0,record
print('Current-main resolver tests passed',flush=True)
