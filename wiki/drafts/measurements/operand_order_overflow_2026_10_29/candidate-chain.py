import os, shutil, subprocess, sys
from pathlib import Path

root = Path.cwd()
out = root/'build/operand-order-overflow'
os.environ['MBX_CACHE_DIR'] = str(root/'build/conjunction-flow/recovery-cache')
os.environ['MBX_LEARNED_INCREMENTAL'] = '0'
patch = subprocess.check_output(['git','diff','HEAD','--','omega-rust'])
assert patch
(out/'candidate.patch').write_bytes(patch)
print('Building guarded contract-query candidate',flush=True)
with (out/'candidate-build.log').open('w',encoding='utf-8') as log:
    subprocess.run(['mbx','build','-p','omega','--locked','--target-dir','build/pass-producer'],stdout=log,stderr=subprocess.STDOUT,check=True)
(out/'candidate').mkdir(exist_ok=True)
for name in ['omega.exe','omega.pdb']:
    shutil.copyfile(root/'build/pass-producer/debug'/name,out/'candidate'/name)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust']) == patch
print('Candidate built; original quality gates',flush=True)
subprocess.run([sys.executable,str(out/'quality.py')],check=True)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust']) == patch
