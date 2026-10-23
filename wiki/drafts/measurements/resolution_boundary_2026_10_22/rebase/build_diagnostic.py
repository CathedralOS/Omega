from pathlib import Path
import subprocess,json,os
scripts=Path(__file__).resolve().parent
output=Path('build/resolved-rebase')
subprocess.run(['python3',str(scripts/'instrument.py')],check=True)
with (output/'oracle-tests.log').open('w') as log:
    subprocess.run(['mbx','nextest','run','-j','2','-p','symbol-resolved-trees','--target-dir','build/gate-cli','--success-output','immediate'],env=dict(os.environ,OMEGA_REBASE_VERIFY='1'),stdout=log,stderr=subprocess.STDOUT,check=True)
with (output/'instrumented-build.jsonl').open('w') as stdout, (output/'instrumented-build.log').open('w') as stderr:
    subprocess.run(['mbx','build','-j','2','-p','omega','--bin','omega','--target-dir','build/gate-cli','--message-format=json-render-diagnostics'],stdout=stdout,stderr=stderr,check=True)
subprocess.run(['python3',str(scripts/'freeze.py'),'instrumented'],check=True)
for name,text in json.loads((output/'before-instrumentation.json').read_text()).items():Path(name).write_text(text)
print('Oracle frozen; production candidate restored',flush=True)
