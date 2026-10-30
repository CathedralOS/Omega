import os, shutil, subprocess, sys
from pathlib import Path
root=Path.cwd(); out=root/'build/first-checking'; warm=root.parent/'pass-ablation'
os.environ['MBX_CACHE_DIR']=str(warm/'build/conjunction-flow/recovery-cache')
os.environ['MBX_LEARNED_INCREMENTAL']='0'
patch=subprocess.check_output(['git','diff','HEAD','--','omega-rust'])
(out/'candidate.patch').write_bytes(patch)
print('Building batch-local operand-resolution candidate',flush=True)
with (out/'candidate-build.log').open('w',encoding='utf-8') as log:
    subprocess.run(['mbx','build','-p','omega','--locked','--target-dir',str(warm/'build/pass-producer')],stdout=log,stderr=subprocess.STDOUT,check=True)
(out/'candidate').mkdir(exist_ok=True)
for name in ['omega.exe','omega.pdb']: shutil.copyfile(warm/'build/pass-producer/debug'/name,out/'candidate'/name)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
print('Running payload/context isolation test',flush=True)
with (out/'unit.log').open('w',encoding='utf-8') as log:
    subprocess.run(['mbx','nextest','run','-p','validation','--lib','-E','test(operand_resolutions_preserve_payload_and_end_with_batch)','--target-dir',str(warm/'build/pass-producer')],stdout=log,stderr=subprocess.STDOUT,check=True)
subprocess.run([sys.executable,str(out/'compare.py')],check=True)
subprocess.run([sys.executable,str(out/'quality.py')],check=True)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
