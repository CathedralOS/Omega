"""Build and freeze the candidate; run the affected representation tests."""
from pathlib import Path
import subprocess
scripts = Path(__file__).resolve().parent
output = Path('build/resolution-integration-final')
with (output/'tests.log').open('w') as log:
    subprocess.run(['mbx','nextest','run','-j','2','-p','symbol-resolved-trees','--target-dir','build/gate-cli'], stdout=log, stderr=subprocess.STDOUT, check=True)
with (output/'candidate-build.jsonl').open('w') as stdout, (output/'candidate-build.log').open('w') as stderr:
    subprocess.run(['mbx','build','-j','2','-p','omega','--bin','omega','--target-dir','build/gate-cli','--message-format=json-render-diagnostics'], stdout=stdout, stderr=stderr, check=True)
subprocess.run(['python3',str(scripts/'freeze.py'),'candidate'],check=True)

subprocess.run(['python3',str(scripts/'setup.py')],check=True)
subprocess.run(['python3',str(scripts/'configure.py')],check=True)
base=(output/'main.txt').read_text().strip()
with (output/'qualification-start.log').open('w') as log:
    subprocess.run(['python3','tools/qualification.py','start','--state',str(output/'qualification.json'),'--base',base,'--config',str(output/'qualification-config.json'),'--summary-output'],stdout=log,stderr=subprocess.STDOUT,check=True)
with (output/'qualification-run.log').open('w') as log:
    subprocess.run(['python3','tools/qualification.py','run-pending','--state',str(output/'qualification.json'),'--revision','0','--base',base,'--summary-output'],stdout=log,stderr=subprocess.STDOUT,check=True)
print('Integrated runtime qualification passed',flush=True)
