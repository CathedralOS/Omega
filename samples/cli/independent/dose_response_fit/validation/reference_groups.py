"""Additional independent freeze: unequal group weights and supplied sigma 1.7.
Uses only the full-variable SciPy reference, never application output.
"""
import hashlib,json,sys
sys.dont_write_bytecode=True
from pathlib import Path
import numpy as np
from scipy.special import expit
saved_arguments=sys.argv;sys.argv=[sys.argv[0]]
from reference import solve
sys.argv=saved_arguments
ROOT=Path(__file__).resolve().parents[1]
name='unequal_activation';sigma=1.7
rows=[];rng=np.random.default_rng(161803)
for dose,count in ((1,1),(3,3),(9,2),(27,4),(81,1)):
 for replicate in range(1,count+1):
  response=12+74*expit(1.4*(np.log(dose)-np.log(12)))+rng.normal(0,.7)
  rows.append([dose,replicate,0,int(round(response*1000))])
for role,response,count in ((1,12,3),(2,86,1)):
 for replicate in range(1,count+1):
  rows.append([0,replicate,role,int(round((response+rng.normal(0,.7))*1000))])
text=f'1 {len(rows)} 1700\n'+''.join(' '.join(map(str,row))+'\n' for row in rows)
fixture=ROOT/'data'/(name+'.txt');oracle=ROOT/'validation'/'oracle_groups.json'
freeze=sys.argv[1:]==['--freeze-groups']
if sys.argv[1:] and not freeze:raise SystemExit('usage: reference_groups.py [--freeze-groups]')
if freeze:
 if fixture.exists() or oracle.exists():raise SystemExit('refusing to overwrite supplemental freeze')
else:assert fixture.read_text()==text
result=solve(rows,1,sigma);result['sigma']=sigma
record={'reference':'independent full-variable bounded TRF and nuisance profile roots from reference.solve',
 'created_before_supplemental_application_execution':True,'fixture_sha256':hashlib.sha256(text.encode()).hexdigest(),'cases':{name:result}}
if freeze:
 fixture.write_text(text);oracle.write_text(json.dumps(record,indent=2)+'\n')
 print('frozen supplemental oracle',hashlib.sha256(oracle.read_bytes()).hexdigest(),flush=True)
else:
 expected=json.loads(oracle.read_text())['cases'][name]
 for key,value in result['parameters'].items():assert abs(value-expected['parameters'][key])<1e-6
 for key,ends in result['intervals'].items():
  for actual,wanted in zip(ends,expected['intervals'][key]):assert actual is not None and wanted is not None and abs(actual-wanted)<1e-6
 print('unequal weights and sigma=1.7 independent reproduction matched')
