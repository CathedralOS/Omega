"""Sequential public-entrypoint validation against pre-execution frozen oracles."""
import argparse, hashlib, json, math, os, re, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
EXPECTED_HASH='7aa66bebbc97a34ff7e00004d43973d9025b6c74fc0499407578dab37ea119e5'
def execute(command,stdin,stem,output):
    result=subprocess.run(command,input=stdin,capture_output=True)
    (output/(stem+'.stdout')).write_bytes(result.stdout)
    (output/(stem+'.stderr')).write_bytes(result.stderr)
    return result

def report(stdout,case,oracle):
    lines=stdout.decode('ascii').splitlines()
    assert lines[0]=='chromatogram_component_fit synthetic unit-spacing NNLS',lines
    rows=dict(line.split('=',1) for line in lines[1:])
    keys=set(case['numeric'])|{'mask','nonidentifiable','status'}
    assert set(rows)==keys and len(lines)==len(keys)+1,rows
    assert rows['mask']==str(case['mask']), rows
    assert rows['nonidentifiable']==str(case['nonidentifiable']).lower(),rows
    assert rows['status']=='ok',rows
    tolerances=oracle['tolerances']
    for key,expected in case['numeric'].items():
        actual=float(rows[key]); assert math.isfinite(actual)
        assert abs(actual-expected)<=tolerances['reported_absolute']+tolerances['reported_relative']*abs(expected),(key,actual,expected)
    return rows

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',required=True)
    parser.add_argument('--route',choices=['check','native','both','invalid'],required=True)
    parser.add_argument('--output',type=Path,default=ROOT/'build'/'validation')
    options=parser.parse_args(); options.output.mkdir(parents=True,exist_ok=True)
    original=(ROOT/'validation'/'oracle.json').read_bytes()
    assert hashlib.sha256(original).hexdigest()=='9f714012c2ac23ac921907be868458f71513fae07e3f096b156353f479f3ec33','historical oracle changed'
    reviewed=(ROOT/'validation'/'oracle_v2.json').read_bytes()
    assert hashlib.sha256(reviewed).hexdigest()=='654c9b334b07237630004572ff0ac6d12f7fd7a30d8e72e7f0198f8f4cf1bb30','reviewed oracle changed'
    raw=(ROOT/'validation'/'oracle_v3.json').read_bytes()
    assert hashlib.sha256(raw).hexdigest()==EXPECTED_HASH,'frozen oracle changed'
    oracle=json.loads(raw); results=[]
    omega=str(Path(options.omega).resolve()); source=str(ROOT/'main.omg')
    if options.route=='check':
        command=[omega,'--check','--target','linux_x86_64',source]
        result=execute(command,b'', 'check',options.output)
        results.append({'command':command,'exit':result.returncode})
        assert result.returncode==0,result.stderr.decode(errors='replace')
    elif options.route=='invalid':
        command=[omega,'--check','--target','linux_x86_64',str(ROOT/'controls'/'intended_invalid'/'invalid.omg')]
        result=execute(command,b'','intended_invalid',options.output)
        text=result.stderr.decode(errors='replace')+result.stdout.decode(errors='replace')
        results.append({'command':command,'exit':result.returncode})
        assert result.returncode==1,text
        # Complete bounds-specific proof, never merely any nonzero outcome.
        expected=(ROOT/'controls'/'intended_invalid'/'expected.txt').read_text().strip()
        assert expected in text,text
        assert not re.search(r'package.*review|custody|unresolved|parse error',text,re.I),text
    else:
        # Compile once for native, then invoke the retained executable for every
        # finite input. Both independently uses the public comparison route.
        if options.route=='native':
            command=[omega,'--target','linux_x86_64','--report-file',str(options.output/'native.compile.report'),source]
            result=execute(command,b'','native.compile',options.output)
            assert result.returncode==0,result.stderr.decode(errors='replace')
            publication=(options.output/'native.compile.report').read_text()
            paths=re.findall(r'^published native output to (.+)$',publication,re.M)
            assert len(paths)==1,publication
            executable=Path(paths[0])
            assert executable.is_file(),executable
            command=[str(executable)]
        else: command=[omega,'run','--both',source]
        for name,case in oracle['cases'].items():
            fixture=ROOT/'data'/(name+'.txt')
            result=execute(command,fixture.read_bytes(),options.route+'.'+name,options.output)
            assert result.returncode==0,(name,result.returncode,result.stderr.decode(errors='replace'))
            report(result.stdout,case,oracle)
            stderr=result.stderr.decode(errors='replace')
            if options.route=='both':
                assert 'DECLINED' not in stderr,(name,stderr)
                assert 'interp exit: 0' in stderr and 'DIVERGENCE' not in stderr,(name,stderr)
            # Independent residual identity and nonnegative stationarity checks.
            values=list(map(float,fixture.read_text().split()))
            rows=dict(line.split('=',1) for line in result.stdout.decode().splitlines()[1:])
            amounts=[float(rows[k]) for k in ('a_amount','b_amount')]
            residuals=[float(rows[f'residual_{i}']) for i in range(5)]
            assert min(amounts)>=0
            for j in range(2):
                gradient=sum(values[5*j+i]*residuals[i] for i in range(5))
                tolerance=0.00001*sum(values[5*j:5*j+5])+0.00001
                assert gradient<=tolerance,(name,'KKT',j,gradient)
                if amounts[j]>0: assert abs(gradient)<=tolerance,(name,'stationarity',j,gradient)
            results.append({'fixture':name,'command':command,'exit':result.returncode,'oracle':'matched'})
        base=(ROOT/'data'/'exact_overlap.txt').read_bytes()
        variants={'eof_after_digit':base.rstrip(), 'crlf':base.replace(b'\n',b'\r\n'),
                  'tabs':b'\t'+base.replace(b' ',b'\t'),
                  'exact_byte_budget':base+b' '*(512-len(base))}
        for name,stdin in variants.items():
            result=execute(command,stdin,options.route+'.'+name,options.output)
            assert result.returncode==0,(name,result.returncode,result.stderr.decode(errors='replace'))
            report(result.stdout,oracle['cases']['exact_overlap'],oracle)
            if options.route=='both': assert 'interp exit: 0' in result.stderr.decode(errors='replace')
            results.append({'fixture':name,'command':command,'exit':result.returncode,'oracle':'exact_overlap matched'})
        result=execute(command,base+b' '*(513-len(base)),options.route+'.over_byte_budget',options.output)
        assert result.returncode==2 and result.stdout.startswith(b'error=input:'),(result.returncode,result.stdout)
        if options.route=='both': assert 'interp exit: 2' in result.stderr.decode(errors='replace')
        results.append({'fixture':'over_byte_budget','command':command,'exit':result.returncode,'oracle':'input rejection matched'})
        for fixture in sorted((ROOT/'data').glob('invalid_*.txt')):
            result=execute(command,fixture.read_bytes(),options.route+'.'+fixture.stem,options.output)
            expected=3 if fixture.stem=='invalid_zero_profile' else 2
            assert result.returncode==expected,(fixture.name,result.returncode,result.stderr.decode(errors='replace'))
            assert result.stdout.startswith(b'error=profile:' if expected==3 else b'error=input:'),result.stdout
            if options.route=='both': assert f'interp exit: {expected}' in result.stderr.decode(errors='replace')
            results.append({'fixture':fixture.stem,'command':command,'exit':result.returncode,'oracle':'matched'})
    (options.output/(options.route+'.results.json')).write_text(json.dumps(results,indent=2)+'\n')
    print(options.route+': '+str(len(results))+' outcomes matched')
if __name__=='__main__': main()
