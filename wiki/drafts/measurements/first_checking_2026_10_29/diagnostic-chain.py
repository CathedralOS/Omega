import os, shutil, subprocess, sys
from pathlib import Path
root=Path.cwd(); out=root/'build/first-checking'; warm=root.parent/'pass-ablation'
os.environ['MBX_CACHE_DIR']=str(warm/'build/conjunction-flow/recovery-cache')
os.environ['MBX_LEARNED_INCREMENTAL']='0'
patch=subprocess.check_output(['git','diff','HEAD','--','omega-rust'])
(out/'diagnostic.patch').write_bytes(patch)
with (out/'diagnostic-build.log').open('w',encoding='utf-8') as log:
    subprocess.run(['mbx','build','-p','omega','--locked','--target-dir',str(warm/'build/pass-producer')],stdout=log,stderr=subprocess.STDOUT,check=True)
(out/'diagnostic').mkdir(exist_ok=True)
for name in ['omega.exe','omega.pdb']: shutil.copyfile(warm/'build/pass-producer/debug'/name,out/'diagnostic'/name)
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==patch
profile=(out/'profile.py').read_text()
profile=profile.replace("root / 'build/preparation-convergence/candidate/omega.exe'", "out / 'diagnostic/omega.exe'")
profile='\n'.join(line for line in profile.splitlines() if not line.startswith('assert benchmark.sha256_file(producer)'))
profile=profile.replace("'phase-'+fixture", "'diagnostic-'+fixture").replace("'phase.json'", "'diagnostic.json'").replace("'phase-inputs.json'", "'diagnostic-inputs.json'")
profile=profile.replace("result = subprocess.run(command, cwd=cwd or root, capture_output=True, timeout=360)", "try:\n            result = subprocess.run(command, cwd=cwd or root, capture_output=True, timeout=360)\n        except subprocess.TimeoutExpired as error:\n            (directory/'timeout.txt').write_bytes((error.stdout or b'')+(error.stderr or b''))\n            raise")
(out/'diagnostic-profile.py').write_text(profile,encoding='utf-8')
subprocess.run([sys.executable,str(out/'diagnostic-profile.py')],check=True)
