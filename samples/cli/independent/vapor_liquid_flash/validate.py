"""Frozen numerical checks on actual native AND captured Terminal stdout."""
import argparse, hashlib, json, re, subprocess, time
from pathlib import Path
from decimal import Decimal as D
import sys
sys.dont_write_bytecode = True
from reference import solve
from closure_reference import coefficients
parser=argparse.ArgumentParser(); parser.add_argument('--omega',required=True)
parser.add_argument('--qualifier',help='optional supported qualify_inputs executable; compile once and isolate every native/Terminal invocation')
options=parser.parse_args(); project=Path(__file__).resolve().parent
raw=(project/'data/oracle.json').read_bytes()
assert hashlib.sha256(raw).hexdigest()==(project/'data/oracle.sha256').read_text().split()[0]
closure_raw=(project/'data/closure_oracle.json').read_bytes()
assert hashlib.sha256(closure_raw).hexdigest()==(project/'data/closure_oracle.sha256').read_text().split()[0]
closure_oracle=json.loads(closure_raw)['cases']
assert coefficients()['cases']==closure_oracle
cases=json.loads(raw)['cases']; out=project/'build/validation'; out.mkdir(parents=True,exist_ok=True)
records=[]
def invoke(name,arguments,data=b'',executable=None):
    command=[executable or options.omega,*arguments]; start=time.monotonic()
    run=subprocess.run(command,input=data,capture_output=True,timeout=240)
    (out/(name+'.stdout')).write_bytes(run.stdout); (out/(name+'.stderr')).write_bytes(run.stderr)
    records.append(dict(name=name,command=command,exit=run.returncode,seconds=time.monotonic()-start,stdout_sha256=hashlib.sha256(run.stdout).hexdigest()))
    (out/'runs.json').write_text(json.dumps(records,indent=2)+'\n')
    return run

def verify(stream, expected):
    text=stream.decode(); status=expected['status']
    if status=='invalid':
        assert text=='INVALID fractions_or_K\n',text; return
    assert 'model: fixed_temperature_pressure supplied_K; ideal equilibrium y=K*x' in text.splitlines()
    assert 'status: '+status in text.splitlines(),text
    if status=='non_identifiable':
        assert 'vapor_fraction: undetermined; all positive-abundance K=1' in text
        assert 'phase_presence: undetermined; if_present_composition: feed; closure_scope: every_admissible_split' in text
        assert 'component:' not in text
        identities=re.findall(r'^component_identity: (\d+) feed=(\S+) closure=(\S+)$',text,re.M)
        assert len(identities)==len(expected['feed']),text
        coefficients=closure_oracle[current_name]
        assert all(D(value)==0 for values in coefficients.values() for value in values)
        for index,row in enumerate(identities):
            assert int(row[0])==index
            assert abs(D(row[1])-D(expected['feed'][index]))<=D('1e-15')
            assert D(row[2])==0
        return
    match=re.search(r'^vapor_fraction: (\d+\.\d+)$',text,re.M); assert match,text
    beta=D(match[1]); assert abs(beta-D(expected['beta']))<=D('1e-8'),text
    rows=re.findall(r'^component: (\d+) feed=(\S+) liquid=(\S+) vapor=(\S+) reconstructed=(\S+) closure=(\S+)$',text,re.M)
    assert len(rows)==len(expected['feed']),text
    sums={'liquid':D(0),'vapor':D(0)}
    for position,row in enumerate(rows):
        index,feed,liquid,vapor,reconstructed,closure=row
        assert int(index)==position
        feed=D(feed); assert abs(feed-D(expected['feed'][position]))<=D('1e-15')
        compositions={}
        for phase,observed in [('liquid',liquid),('vapor',vapor)]:
            oracle=expected[phase]
            if oracle is None: assert observed=='absent',row
            else:
                value=D(observed); assert 0<=value<=1,row
                assert abs(value-D(oracle[position]))<=D('1e-8'),(phase,row,oracle)
                sums[phase]+=value; compositions[phase]=value
        reconstructed=D(reconstructed); closure=D(closure)
        assert 0<=closure<=D('1e-10'),row
        assert abs(reconstructed-feed)<=D('1e-10'),row
        mixture=(1-beta)*compositions.get('liquid',D(0))+beta*compositions.get('vapor',D(0))
        assert abs(mixture-feed)<=D('1e-10'),row
        assert abs(abs(reconstructed-feed)-closure)<=D('2e-15'),row
        if len(compositions)==2:
            tokens=current_input.split(); ratio=D(tokens[2+2*position])
            assert abs(compositions['vapor']-ratio*compositions['liquid'])<=D('3e-14'),row
    for phase in sums:
        if expected[phase] is not None: assert abs(sums[phase]-1)<=D('1e-8'),sums

check=invoke('check',['--check',str(project/'main.omg')]); assert check.returncode==0,check.stderr.decode()
invalid=invoke('source_invalid',['--check',str(project/'controls/component_bound/main.omg')])
bound_diagnostic=(project/'controls/component_bound/expected.txt').read_bytes().strip()
assert invalid.returncode==1 and bound_diagnostic in invalid.stderr,invalid.stderr.decode()
qualified=None
if options.qualifier:
    qualified=out/'qualified'; inputs=[]
    for case in cases:
        data=case['input'].encode()
        assert (project/'data'/(case['name']+'.txt')).read_bytes()==data
        expected=case['expected']
        if expected['exit'] in (0,3):
            tokens=case['input'].split(); assert solve(tokens[1::2],tokens[2::2])==expected
        directory=qualified/case['name']; directory.mkdir(parents=True,exist_ok=True)
        path=directory/'input.txt'; path.write_bytes(data); inputs.append(str(path))
    batch=invoke('qualify',[str(project/'main.omg'),str(qualified),*inputs],executable=options.qualifier)
    assert batch.returncode==0,batch.stderr.decode()
for case in cases:
    current_name=case['name']; current_input=case['input']; expected=case['expected']
    assert (project/'data'/(case['name']+'.txt')).read_bytes()==current_input.encode()
    if expected['exit'] in (0,3):
        tokens=current_input.split(); independent=solve(tokens[1::2],tokens[2::2]); assert independent==expected
    if qualified is None:
        run=invoke(case['name'],['run','--both','--keep',str(project/'main.omg')],current_input.encode())
        diagnostics=run.stderr.decode(); assert run.returncode==expected['exit'],diagnostics
        assert f"interp exit: {expected['exit']}" in diagnostics.splitlines(),diagnostics
        assert 'DECLINED' not in diagnostics and 'DIVERGENCE' not in diagnostics,diagnostics
        match=re.search(r'^build dir kept: (.+)$',diagnostics,re.M); assert match,diagnostics
        retained=Path(match[1]); native=run.stdout; terminal=(retained/'terminal.stdout').read_bytes()
        assert not (retained/'terminal.stderr').read_bytes()
    else:
        retained=qualified/case['name']; native=(retained/'native.stdout').read_bytes()
        terminal=(retained/'terminal.stdout').read_bytes()
        assert int((retained/'native.status').read_text())==expected['exit']
        status=(retained/'terminal.status').read_text()
        assert status.startswith('ExternalCompletion(') and 'kind: ProcessExit,' in status,status
        assert re.search(r'\breturn_code: (-?\d+),',status).group(1)==str(expected['exit']),status
        assert not (retained/'native.stderr').read_bytes()
        assert not (retained/'terminal.stderr').read_bytes()
        (out/(case['name']+'.stdout')).write_bytes(native)
    (out/(case['name']+'.terminal.stdout')).write_bytes(terminal)
    verify(native,expected); verify(terminal,expected)
    print(case['name'],'native and captured Terminal numerical values verified',flush=True)
print(len(cases),'runtime cases, source-invalid component bound verified')
