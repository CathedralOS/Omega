import os,subprocess,sys,shutil
from pathlib import Path
root=Path.cwd();out=root/'build/source-resolution'
os.environ['MBX_CACHE_DIR']=str(root/'build/conjunction-flow/recovery-cache');os.environ['MBX_LEARNED_INCREMENTAL']='0'
patch=subprocess.check_output(['git','diff','--','omega-rust']);(out/'candidate.patch').write_bytes(patch)
def run(name,command):
 print(name,flush=True)
 with (out/(name+'.log')).open('w',encoding='utf-8') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
run('candidate-build',['mbx','build','-p','omega','--locked','--target-dir','build/pass-producer'])
(out/'candidate').mkdir(exist_ok=True);shutil.copyfile(root/'build/pass-producer/debug/omega.exe',out/'candidate/omega.exe')
assert subprocess.check_output(['git','diff','--','omega-rust'])==patch
run('quality',[sys.executable,str(out/'quality.py')])
run('comparison',[sys.executable,'wiki/drafts/measurements/checking_simplification_2026_10_28/compare.py','build/flow-evaluation/candidate/omega.exe',str(out/'candidate/omega.exe'),str(out/'comparison')])
print('Candidate checks and comparison completed.',flush=True)
