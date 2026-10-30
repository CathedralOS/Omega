import os, shutil, subprocess, sys
from pathlib import Path
root=Path.cwd(); out=root/'build/body-unfolding'
os.environ['MBX_CACHE_DIR']=str(root.parent/'pass-ablation/build/conjunction-flow/recovery-cache')
os.environ['MBX_LEARNED_INCREMENTAL']='0'
target=root/'build/remaining-producer'
patch=subprocess.check_output(['git','diff','HEAD','--','omega-rust'])
(out/'guards.patch').write_bytes(patch)
print('Building selected-contract guards in worktree-owned target directory',flush=True)
with (out/'guards-build.log').open('w',encoding='utf-8') as log:
    subprocess.run(['mbx','build','-p','omega','--locked','--target-dir',str(target)],stdout=log,stderr=subprocess.STDOUT,check=True)
(out/'guards').mkdir(exist_ok=True)
for name in ['omega.exe','omega.pdb']:
    source=target/'debug'/name
    if source.exists(): shutil.copyfile(source,out/'guards'/name)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
subprocess.run([sys.executable,str(out/'guards-profile.py')],check=True)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
