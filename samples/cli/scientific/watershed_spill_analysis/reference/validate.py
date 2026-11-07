"""Serial public-entry validation against immutable byte and exit oracles."""
from pathlib import Path
import argparse, hashlib, json, os, subprocess, sys
sys.dont_write_bytecode = True
import oracle
parser=argparse.ArgumentParser()
parser.add_argument('--omega', required=True)
options=parser.parse_args()
root=oracle.ROOT
repository=root.parents[3]
output=root/'build/validation'; output.mkdir(parents=True,exist_ok=True)
manifest=json.loads((root/'reference/frozen.json').read_text())
artifacts=output/'artifacts'; artifacts.mkdir(exist_ok=True)
environment=os.environ.copy(); environment['TMPDIR']=str(artifacts)
environment['OMEGA_LIBRARY_ROOT']=str(repository/'source/library')
records=[]
def run(name,arguments,data=b''):
    command=[str(Path(options.omega).resolve()),*arguments]
    before=set(artifacts.glob('omega-probe-*'))
    result=subprocess.run(command,input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=180,cwd=repository,env=environment)
    (output/f'{name}.stdout').write_bytes(result.stdout)
    (output/f'{name}.stderr').write_bytes(result.stderr)
    record={'name':name,'stdin_sha256':hashlib.sha256(data).hexdigest(),'input_fixture':str(root/'fixtures'/f'{name}.txt') if name in manifest['cases'] else None,'command':command,'exit':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()}
    record['artifact_directories']=[str(p) for p in sorted(set(artifacts.glob('omega-probe-*'))-before)]
    records.append(record); (output/'results.json').write_text(json.dumps(records,indent=2)+'\n')
    return result
# Check first; comparison compiles through the same complete native route.
result=run('check',['--check',str(root/'main.omg')]); assert result.returncode==0, result.stderr.decode(errors='replace')
for name,case in manifest['cases'].items():
    data=(root/'fixtures'/f'{name}.txt').read_bytes()
    assert hashlib.sha256(data).hexdigest()==case['input_sha256']
    expected=(root/'fixtures'/f'{name}.expected.txt').read_bytes()
    assert hashlib.sha256(expected).hexdigest()==case['stdout_sha256']
    if case['exit']==0: assert oracle.expected(data.decode()).encode()==expected
    result=run(name,['run','--both','--keep',str(root/'main.omg')],data)
    assert result.returncode==case['exit'], (name,result.returncode,result.stderr.decode(errors='replace'))
    assert result.stdout==expected, (name,'stdout differs',result.stdout,expected)
    assert f'interp exit: {case["exit"]}'.encode() in result.stderr and b'DECLINED' not in result.stderr and b'DIVERGENCE' not in result.stderr, (name,'Terminal comparison unavailable',result.stderr)
    print(name, 'exact stdout/exit; native and Terminal agree',flush=True)
result=run('intended_invalid',['--check',str(root/'intended_invalid/invalid.omg')])
assert result.returncode==1, ('intended-invalid must reject',result.returncode)
assert b'index' in result.stderr.lower() and (b'bound' in result.stderr.lower() or b'prove' in result.stderr.lower()), result.stderr
print('intended-invalid: independently rejected for index bounds')
print(f'{len(manifest["cases"])} cases validated; tolerance 0; {output}')
