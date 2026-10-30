from pathlib import Path
import gzip,json,subprocess
ROOT=Path.cwd();WORK=ROOT/'build/proof-profile/source-checking-cpu'
meta=subprocess.check_output(['gh','run','view','41002093180','--repo','CathedralOS/Omega','--json','status,conclusion,headSha,url,jobs'])
data=json.loads(meta);assert data['status']=='completed' and data['conclusion']=='success' and data['headSha']=='cec7b6657299525e62aa6fc2168d3768b4d33b6a'
(WORK/'post-merge-smoke.json').write_bytes(meta+b'\n')
logs=subprocess.check_output(['gh','run','view','41002093180','--repo','CathedralOS/Omega','--log'])
(WORK/'post-merge-smoke.log.gz').write_bytes(gzip.compress(logs,mtime=0))
lines=[line for line in logs.decode('utf-8','replace').splitlines() if any(t in line.lower() for t in ['pilot result','smoke result','passed','failed','compiler_pilot.py','reject','summary'])]
(WORK/'post-merge-smoke-extract.txt').write_text('\n'.join(lines)+'\n',encoding='utf-8')
print('\n'.join(lines[-25:]))
