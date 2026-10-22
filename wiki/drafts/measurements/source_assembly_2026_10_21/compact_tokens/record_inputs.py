from pathlib import Path
import hashlib,json,subprocess
f=Path('build/compact-token-ingress')
inputs=sorted(set((f/'inputs.txt').read_text().splitlines()+(f/'token-inputs.txt').read_text().splitlines()))
record={str(Path(p).resolve().relative_to(Path.cwd())):hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in inputs}
(f/'input-hashes.json').write_text(json.dumps(record,indent=2)+'\n')
identities={}
for name in ['omega-baseline','omega-candidate','harness-baseline','harness-candidate','harness-all-baseline','harness-all-candidate','token-probe-baseline','token-probe-candidate']:
 p=f/name;identities[name]={'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size}
(f/'identities.json').write_text(json.dumps({'integrated_base':(f/'base.txt').read_text().strip(),'token_baseline_base':'2faaf46a51bbeab1ff003b0d0a8aaabac340420f','binaries':identities},indent=2)+'\n')
