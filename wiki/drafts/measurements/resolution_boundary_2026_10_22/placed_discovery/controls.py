"""Observe both engines for the placed-view positives in all three modes."""
from pathlib import Path
import json
import os
import subprocess

output = Path('build/placed-discovery')
records=[]
for subject in ('returned','forwarded'):
    for mode in ('baseline','full','skip'):
        binary = Path('build/resolution-measurement/omega-baseline') if mode=='baseline' else output/'omega-instrumented'
        environment=dict(os.environ)
        for key in ('OMEGA_RESOLUTION_MEASURE','OMEGA_PLACED_DISCOVERY_MEASURE','OMEGA_PLACED_DISCOVERY_MODE'):
            environment.pop(key,None)
        if mode!='baseline':
            environment['OMEGA_PLACED_DISCOVERY_MODE']=mode
        command=[str(binary.resolve()),'run','--both',f'tests/omega/pass/layouts/placed_reference_results/{subject}/main.omg']
        result=subprocess.run(command,env=environment,stdin=subprocess.DEVNULL,capture_output=True,text=True)
        row=dict(subject=subject,mode=mode,command=command,exit=result.returncode,stdout=result.stdout,stderr=result.stderr)
        records.append(row)
        (output/'controls.json').write_text(json.dumps(records,indent=2)+'\n')
        assert result.returncode==0 and 'native exit 0' in result.stderr and 'interp exit: 0' in result.stderr,row
        print(subject,mode,'native 0; interpreter 0',flush=True)
