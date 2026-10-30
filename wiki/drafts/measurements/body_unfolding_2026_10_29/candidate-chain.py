import os,shutil,subprocess,sys
from pathlib import Path
root=Path.cwd();out=root/'build/body-unfolding';target=root/'build/remaining-producer'
os.environ['MBX_CACHE_DIR']=str(root.parent/'pass-ablation/build/conjunction-flow/recovery-cache')
os.environ['MBX_LEARNED_INCREMENTAL']='0'
patch=subprocess.check_output(['git','diff','HEAD','--','omega-rust'])
(out/'candidate.patch').write_bytes(patch)
assert b'05.unfold.' not in patch and b'05.guard.' not in patch
print('Building guard-query operand reuse candidate',flush=True)
with (out/'candidate-build.log').open('w',encoding='utf-8') as log:
    subprocess.run(['mbx','build','-p','omega','--locked','--target-dir',str(target)],stdout=log,stderr=subprocess.STDOUT,check=True)
(out/'candidate').mkdir(exist_ok=True)
for name in ['omega.exe','omega.pdb']:
    path=target/'debug'/name
    if path.exists():shutil.copyfile(path,out/'candidate'/name)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
subprocess.run([sys.executable,str(out/'compare.py')],check=True)
subprocess.run([sys.executable,str(out/'quality.py')],check=True)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
