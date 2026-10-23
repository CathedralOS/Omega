"""Freeze one diagnostic binary under the existing build-slot protocol."""
from pathlib import Path
import json
import subprocess

scripts = Path(__file__).resolve().parent
output = Path('build/placed-discovery')
assert not (output/'omega-instrumented').exists()
subprocess.run(['python3',str(scripts/'instrument.py')],check=True)
command = ['mbx','build','-j','2','-p','omega','--bin','omega','--target-dir','build/gate-cli',
           '--message-format=json-render-diagnostics']
with (output/'instrumented-build.jsonl').open('w') as stdout, (output/'instrumented-build.log').open('w') as stderr:
    subprocess.run(command,stdout=stdout,stderr=stderr,check=True)
subprocess.run(['python3',str(scripts/'freeze.py'),'instrumented'],check=True)
subprocess.run(['git','restore','--',*json.loads((output/'instrumented-paths.json').read_text())],check=True)
assert not subprocess.check_output(['git','diff','HEAD','--','omega-rust'])
print('Diagnostic binary frozen; source restored',flush=True)
