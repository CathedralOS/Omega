"""Observe both engines and preserve the length reject twin's diagnostic."""
import json
import os
from pathlib import Path
import subprocess

folder = Path('build/resolution-measurement').resolve()
records = []
for name in ('baseline', 'instrumented'):
    binary = folder / ('omega-' + name)
    environment = dict(os.environ)
    environment.pop('OMEGA_RESOLUTION_MEASURE', None)
    if name == 'instrumented':
        environment['OMEGA_RESOLUTION_MEASURE'] = '1'
    for kind, arguments in (
        ('both', ['run','--both','samples/cli/facts/length/local/main.omg']),
        ('invalid', ['--check','samples/cli/facts/length/local/invalid/main.omg']),
    ):
        command = [str(binary), *arguments]
        result = subprocess.run(command, env=environment, stdin=subprocess.DEVNULL,
                                text=True,capture_output=True)
        clean = '\n'.join(line for line in result.stderr.splitlines() if not line.startswith('RESMEASURE\t'))
        row = dict(producer=name, kind=kind, command=command, exit=result.returncode,
                   stdout=result.stdout, stderr=result.stderr, diagnostic=clean)
        records.append(row)
        (folder/'controls.json').write_text(json.dumps(records,indent=2)+'\n')
        if kind == 'both':
            assert result.returncode==0 and 'interp exit: 0' in clean and 'native' in clean, clean
        else:
            assert result.returncode > 0 and 'cannot prove index' in clean, clean
        print(name,kind,result.returncode,clean[-1200:],flush=True)
invalid = [row for row in records if row['kind']=='invalid']
assert invalid[0]['diagnostic']==invalid[1]['diagnostic']
