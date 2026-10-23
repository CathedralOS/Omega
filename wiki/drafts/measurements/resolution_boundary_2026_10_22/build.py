"""Build and freeze both producers serially under one existing build-slot lease.

Run via: python3 tools/build_slot.py -- python3 <this-script>
The compiler source is restored after successful builds. Instrumentation and
effective build profiles remain available in the measurement output directory.
"""
from pathlib import Path
import json
import subprocess
import hashlib
import sys

scripts = Path(__file__).resolve().parent
output = Path('build/resolution-measurement')
assert not subprocess.check_output(['git','diff','HEAD','--','omega-rust'])
resume = sys.argv[1:] == ['--resume']
assert not sys.argv[1:] or resume
if resume:
    baseline = json.loads((output/'baseline-producer.json').read_text())
    assert hashlib.sha256((output/'omega-baseline').read_bytes()).hexdigest() == baseline['sha256']
else:
    assert not (output/'omega-baseline').exists(), 'Preserve the existing capture before starting a new one'

def run(script, *arguments):
    subprocess.run(['python3',str(scripts/script),*arguments],check=True)

for name in (('instrumented',) if resume else ('baseline','instrumented')):
    if name == 'instrumented':
        run('instrument.py')
        (output/'instrumentation.patch').write_bytes(subprocess.check_output(['git','diff','--','omega-rust']))
    command = ['mbx','build','-j','2','-p','omega','--bin','omega','--target-dir','build/gate-cli',
               '--message-format=json-render-diagnostics']
    print('Building',name,flush=True)
    with (output/(name+'-build.jsonl')).open('w') as stdout, (output/(name+'-build.log')).open('w') as stderr:
        subprocess.run(command,stdout=stdout,stderr=stderr,check=True)
    run('freeze.py',name)
paths=json.loads((output/'instrumented-paths.json').read_text())
subprocess.run(['git','restore','--',*paths],check=True)
assert not subprocess.check_output(['git','diff','--','omega-rust'])
print('Both immutable producers ready; temporary compiler probes removed',flush=True)
run('capture.py')
run('controls.py')
run('summarize.py')
