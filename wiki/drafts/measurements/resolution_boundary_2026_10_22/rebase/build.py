"""Build and freeze the candidate; run the affected representation tests."""
from pathlib import Path
import subprocess
scripts = Path(__file__).resolve().parent
output = Path('build/resolved-rebase')
with (output/'tests.log').open('w') as log:
    subprocess.run(['mbx','nextest','run','-j','2','-p','symbol-resolved-trees','--target-dir','build/gate-cli'], stdout=log, stderr=subprocess.STDOUT, check=True)
with (output/'candidate-build.jsonl').open('w') as stdout, (output/'candidate-build.log').open('w') as stderr:
    subprocess.run(['mbx','build','-j','2','-p','omega','--bin','omega','--target-dir','build/gate-cli','--message-format=json-render-diagnostics'], stdout=stdout, stderr=stderr, check=True)
subprocess.run(['python3',str(scripts/'freeze.py'),'candidate'],check=True)
