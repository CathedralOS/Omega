from pathlib import Path
import subprocess,json,hashlib,sys
cli=Path(sys.argv[1]).resolve();label=sys.argv[2];root=Path('samples/cli/facts/nested_machine_invocation/private_arithmetic/wrapping_index/controls');rows=[]
for p in sorted(root.glob('*/main.omg')):
 bad=(p.parent/'sample.json').exists();command=[str(cli),'--check',str(p)] if bad else [str(cli),'run','--both',str(p)];r=subprocess.run(command,input='',capture_output=True,text=True,timeout=180);row=dict(name=p.parent.name,invalid=bad,command=command,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),exit=r.returncode,stdout=r.stdout,stderr=r.stderr);rows.append(row);print(p.parent.name,r.returncode,r.stderr.strip(),flush=True)
Path('build/wrapping-integration/controls.json').write_text(json.dumps(rows,indent=2)+'\n')
