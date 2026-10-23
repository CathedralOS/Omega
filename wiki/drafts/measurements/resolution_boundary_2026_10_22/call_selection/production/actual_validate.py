"""Replay immutable products and authored controls with the production compiler."""
import hashlib,json,os,subprocess
from pathlib import Path
folder=Path('build/placed-result-selection-production')
binary=(folder/'omega-candidate').resolve()
environment=os.environ.copy();environment.pop('OMEGA_CALL_RESULT_EXPERIMENT',None)
def run(command):
 result=subprocess.run(command,env=environment,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
 return dict(command=command,exit=result.returncode,output=result.stdout)
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
expected=json.loads(Path('build/placed-result-selection-integrated/full/runtime.json').read_text())['observations']
setup=json.loads((folder/'runtime.json').read_text())['observations']
observations=[]
for row in setup:
 if 'native' not in row:continue
 baseline=next(record for record in expected if record['name']==row['name'])
 image=(folder/('native-'+row['name'])).resolve()
 assert digest(image)==row['native_sha256']==baseline['native_sha256'],row['name']
 native=run([str(image)]);assert native['exit']==0,native
 both=run([str(binary),'run','--both',row['subject']])
 assert both['exit']==0 and 'native exit 0' in both['output'] and 'interp exit: 0' in both['output'],both
 observations.append(dict(name=row['name'],native=native,both=both))
 print(row['name'],'native/Terminal agreement and baseline image',flush=True)
# Retain the exact source refusals, including known baseline failures. Positive
# check-only cases also pin their original outcome; labels alone never bless them.
checks=json.loads(Path('build/placed-result-selection-integrated/observations.json').read_text())['observations']
for row in checks:
 if row['mode']!='full':continue
 command=[str(binary),'--check',str(Path(row['subject']).resolve())]
 actual=run(command)
 assert actual['exit']==row['exit'],(row['name'],actual)
 expected_output=''.join(line for line in row['output'].splitlines(keepends=True) if not line.startswith('CALLRESULT\t'))
 assert actual['output']==expected_output,(row['name'],actual['output'],expected_output)
 observations.append(dict(name=row['name'],check=actual))
 print(row['name'],'baseline checking outcome retained',flush=True)
(folder/'validation.json').write_text(json.dumps(observations,indent=2)+'\n')
print('Production replay passed',flush=True)
