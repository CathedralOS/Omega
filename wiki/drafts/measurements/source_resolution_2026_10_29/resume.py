import sys,subprocess
from pathlib import Path
out=Path('build/source-resolution')
for name,command in [('quality',[sys.executable,str(out/'quality.py')]),('comparison',[sys.executable,'wiki/drafts/measurements/checking_simplification_2026_10_28/compare.py','build/flow-evaluation/candidate/omega.exe',str(out/'candidate/omega.exe'),str(out/'comparison')])]:
 print(name,flush=True)
 with (out/(name+'.log')).open('a') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
