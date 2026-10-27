import hashlib,json,subprocess
from pathlib import Path
cli=str(Path('build/saved-case-calls/producer-4/omega').resolve());root=Path('build/saved-case-calls/probes');records=[]
for name in ['fresh_tag','negated_receiver','negated_receiver_stale','trailing_write']:
 p=Path('tests/omega/fail/control_flow/saved_case_predicate_receiver_trailing_write_rejected/main.omg') if name=='trailing_write' else root/name/'main.omg'
 bad=name.endswith('stale') or name=='trailing_write';command=[cli,'--check',str(p)] if bad else [cli,'run','--both',str(p)]
 r=subprocess.run(command,input='',capture_output=True,text=True,timeout=180);row=dict(name=name,command=command,source_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),exit=r.returncode,stdout=r.stdout,stderr=r.stderr);records.append(row);print(row,flush=True)
Path('build/saved-case-calls/probes-4.json').write_text(json.dumps(records,indent=2)+'\n')
