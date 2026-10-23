"""Capture the final production replay once through pinned qualification."""
import json,os,subprocess,time
from pathlib import Path
folder=Path('build/placed-result-selection-production')
os.environ['CARGO_TARGET_DIR']=str(Path('build/gate-cli').resolve());os.environ['CARGO_BUILD_JOBS']='2'
start=time.monotonic()
while not (folder/'tests-current.json').exists():
 if time.monotonic()-start>3600:raise TimeoutError('current resolver tests unfinished')
 time.sleep(1)
assert json.loads((folder/'tests-current.json').read_text())['exit']==0
subprocess.run(['python3',str(folder/'configure.py')],check=True)
state=folder/'qualification-state.json'
assert not state.exists(),'Do not replace an existing qualification'
base=(folder/'main-base.txt').read_text().strip()
subprocess.run(['python3','tools/qualification.py','start','--state',str(state),'--base',base,'--config',str(folder/'qualification-config.json'),'--summary-output'],check=True)
revision=json.loads(state.read_text())['revision']
subprocess.run(['python3','tools/qualification.py','run-pending','--state',str(state),'--revision',str(revision),'--base',base,'--summary-output'],check=True)
