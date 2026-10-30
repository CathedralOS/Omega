import subprocess,json,time,hashlib
from pathlib import Path
out=Path('build/flow-evaluation');root=Path.cwd();paths={v:root/'build/flow-evaluation/comparison'/name/'build/omega-program.exe' for v,name in [('A','dispatch-02-A-measured'),('B','dispatch-03-B-measured')]};rows=[]
for v in 'ABBAAB':
    path=paths[v];start=time.perf_counter()
    try:
        p=subprocess.run([str(path)],cwd=root,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
        row={'variant':v,'exit':p.returncode,'stdout_hex':p.stdout.hex(),'stderr_hex':p.stderr.hex()}
    except subprocess.TimeoutExpired:row={'variant':v,'timeout_seconds':10}
    row.update(wall_ms=(time.perf_counter()-start)*1000,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),path=str(path));rows.append(row);(out/'native-replays.json').write_text(json.dumps(rows,indent=2));print(row,flush=True)
    if 'timeout_seconds' in row:break
