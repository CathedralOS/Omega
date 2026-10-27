from pathlib import Path
import concurrent.futures,json,subprocess
cli='build/generic-case-observation/producer-5/omega';root=Path('build/generic-case-observation/semantic-controls')
def run(path):
 positive=path.parent.name in ['ordered','reversed','inferred'];command=[cli,'run','--both',str(path)] if positive else [cli,'--check',str(path)];r=subprocess.run(command,text=True,capture_output=True,timeout=180);row=dict(name=path.parent.name,positive=positive,command=command,exit=r.returncode,stdout=r.stdout,stderr=r.stderr);print(json.dumps(row),flush=True);return row
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:rows=list(pool.map(run,sorted(root.glob('*/main.omg'))))
(root/'results-5.json').write_text(json.dumps(rows,indent=2)+'\n')
