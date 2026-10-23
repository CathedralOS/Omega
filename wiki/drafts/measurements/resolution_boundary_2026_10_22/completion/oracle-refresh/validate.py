import json,hashlib,subprocess
from pathlib import Path
out=Path('build/deferred-completion-storage-landing')
for row in json.loads((out/'products.json').read_text()):
 image=Path(row['path']);assert hashlib.sha256(image.read_bytes()).hexdigest()==row['sha256']
 result=subprocess.run([str(image)],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=30)
 assert result.returncode==row['native_exit']==0 and result.stdout==row['native_output'],row
 print(row['name'],'frozen native exit 0',flush=True)
subprocess.run(['python3','-u',str(Path('build/deferred-completion-storage-oracle/capture.py')),'production'],check=True)
