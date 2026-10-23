import os,subprocess,sys,time
from pathlib import Path
sys.path.insert(0,str(Path('tools').resolve()))
import build_slot
build_slot.POLL_SECONDS=.05
out=Path('build/placed-result-selection-production')
os.environ['CARGO_TARGET_DIR']=str(Path('build/gate-cli').resolve());os.environ['CARGO_BUILD_JOBS']='2'
start=time.monotonic()
while not Path('build/placed-result-selection-final/validation.json').exists():
 if time.monotonic()-start>1800:raise TimeoutError('prior coherent replay incomplete')
 time.sleep(1)
assert not subprocess.check_output(['git','status','--porcelain'])
base=(out/'incoming-main.txt').read_text().strip()
subprocess.run(['git','rebase',base],check=True)
(out/'base.txt').write_text(subprocess.check_output(['git','rev-parse','HEAD'],text=True))
(out/'main-base.txt').write_text(base+'\n')
print('Integrated current main; building production',flush=True)
with build_slot.hold():
 with (out/'candidate-build.jsonl').open('w') as stdout,(out/'candidate-build.log').open('w') as stderr:
  result=subprocess.run(['mbx','build','-j','2','-p','omega','--bin','omega','--message-format=json-render-diagnostics'],stdout=stdout,stderr=stderr)
 assert result.returncode==0,result.returncode
subprocess.run(['python3',str(out/'freeze.py'),'candidate'],check=True)
subprocess.run(['python3','-u',str(out/'runtime.py'),str(out),'production',str(out/'omega-candidate')],check=True)
subprocess.run(['python3','-u',str(out/'validate.py')],check=True)
print('Current-main production replay passed',flush=True)
