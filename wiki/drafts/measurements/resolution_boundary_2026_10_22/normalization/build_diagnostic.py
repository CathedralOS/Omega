from pathlib import Path
import subprocess
scripts = Path(__file__).resolve().parent
output = Path('build/resolved-normalization')
subprocess.run(['python3',str(scripts/'instrument.py')],check=True)
with (output/'instrumented-build.jsonl').open('w') as stdout, (output/'instrumented-build.log').open('w') as stderr:
    subprocess.run(['mbx','build','-j','2','-p','omega','--bin','omega','--target-dir','build/gate-cli','--message-format=json-render-diagnostics'],stdout=stdout,stderr=stderr,check=True)
subprocess.run(['python3',str(scripts/'freeze.py'),'instrumented'],check=True)
owner=Path('omega-rust/psi/compilation/representations/symbol-resolved-trees/src/symbol_resolved_trees.rs')
owner.write_bytes((output/'owner-before-instrumentation.rs').read_bytes())
owner.with_suffix('').joinpath('normalization_oracle.rs').unlink()
print('Oracle frozen; production candidate restored',flush=True)
