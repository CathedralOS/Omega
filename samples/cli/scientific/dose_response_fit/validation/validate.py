"""Validate actual numerical reports against the pre-execution independent freeze."""
import argparse, hashlib, json, math, os, re, subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
ORACLE_SHA256='9ec778992f7f88bdf7282a3d245b52f6c3d901a4121162d7cc2fc9f757a96187'
SUPPLEMENTAL_SHA256='d2ce6149695f22132dd2570927b625529e0d50809d46c3fa791f16e7a9bd59d2'
def load_oracle():
    raw=(ROOT/'validation'/'oracle.json').read_bytes()
    assert hashlib.sha256(raw).hexdigest()==ORACLE_SHA256
    oracle=json.loads(raw)
    freeze=json.loads((ROOT/'validation'/'FREEZE.json').read_text())
    for file,wanted in freeze['promoted_files'].items():
        if file.startswith('data/') or file.endswith('oracle.json'):
            assert hashlib.sha256((ROOT/file).read_bytes()).hexdigest()==wanted,file
    raw=(ROOT/'validation'/'oracle_groups.json').read_bytes()
    assert hashlib.sha256(raw).hexdigest()==SUPPLEMENTAL_SHA256
    supplement=json.loads(raw)
    for name in supplement['cases']:
        assert hashlib.sha256((ROOT/'data'/(name+'.txt')).read_bytes()).hexdigest()==supplement['fixture_sha256']
    oracle['cases'].update(supplement['cases'])
    return oracle

def execute(command, stdin, output, name, timeout):
    result=subprocess.run(command,input=stdin,capture_output=True,timeout=timeout)
    (output/(name+'.stdout')).write_bytes(result.stdout)
    (output/(name+'.stderr')).write_bytes(result.stderr)
    (output/(name+'.exit')).write_text(str(result.returncode)+'\n')
    return result

def check_report(stdout,name,oracle):
    lines=stdout.decode('ascii').splitlines()
    assert lines[0]=='dose_response_fit synthetic bounded 4PL',lines
    pairs=[line.split('=',1) for line in lines[1:]]
    assert all(len(pair)==2 for pair in pairs),pairs
    values=dict(pairs); assert len(values)==len(pairs),'duplicate report keys'
    assert values['status']=='ok' and values['converged']=='true' and values['identifiable']=='true',values
    expected=oracle['cases'][name]; tolerance=oracle['tolerances']; parameters=expected['parameters']
    def close(key,target,limit):
        value=float(values[key]);assert math.isfinite(value),(name,key,value)
        assert abs(value-target)<=limit,(name,key,value,target,limit)
    for key,value in parameters.items():close(key,value,tolerance['parameters_absolute'])
    close('half_dose',expected['half_dose'],tolerance['parameters_absolute'])
    close('sse',expected['sse'],tolerance['sse_absolute'])
    close('profile_confidence',.95,1e-12);close('sigma',expected.get('sigma',1),1e-12)
    close('profile_cutoff',expected['cutoff'],tolerance['sse_absolute'])
    assert values['half_dose_kind']==('EC50' if 'activation' in name else 'IC50')
    close('signed_slope',parameters['slope']*(1 if 'activation' in name else -1),tolerance['parameters_absolute'])
    for position,key in enumerate(('lowdose_asymptote','highdose_asymptote','log_half_dose','slope')):
        lower,upper=expected['intervals'][key]; assert lower is not None and upper is not None
        close(key+'_lower',lower,tolerance['intervals_absolute']);close(key+'_upper',upper,tolerance['intervals_absolute'])
        assert float(values[key+'_lower'])<float(values[key])<float(values[key+'_upper'])
        for side in range(2):close(f'profile_endpoint_sse_{2*position+side}',expected['cutoff'],.002)
    close('half_dose_lower',math.exp(expected['intervals']['log_half_dose'][0]),.025)
    close('half_dose_upper',math.exp(expected['intervals']['log_half_dose'][1]),.025)
    for position,(prediction,residual) in enumerate(zip(expected['predictions'],expected['residuals'])):
        close(f'prediction_{position:02d}',prediction,tolerance['residual_absolute'])
        close(f'residual_{position:02d}',residual,tolerance['residual_absolute'])
    actual_residual=[float(values[f'residual_{position:02d}']) for position in range(len(expected['residuals']))]
    assert abs(sum(v*v for v in actual_residual)-float(values['sse']))<.00002
    assert 0<float(values['iterations'])<=2000
    return values

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--omega',required=True,type=Path)
    parser.add_argument('--route',required=True,choices=['check','native','both','invalid'])
    parser.add_argument('--timeout',type=int,default=900)
    parser.add_argument('--output',type=Path,default=ROOT/'build'/'validation')
    options=parser.parse_args();output=options.output.resolve();output.mkdir(parents=True,exist_ok=True)
    oracle=load_oracle()
    omega=str(options.omega.resolve());source=str(ROOT/'main.omg');results=[]
    if options.route=='check':
        command=[omega,'--check',source];result=execute(command,b'',output,'check',options.timeout)
        assert result.returncode==0,result.stderr.decode(errors='replace');results.append({'command':command,'exit':0})
    elif options.route=='invalid':
        command=[omega,'--check',str(ROOT/'controls'/'intended_invalid'/'invalid.omg')]
        result=execute(command,b'',output,'intended_invalid',options.timeout)
        text=(result.stderr+result.stdout).decode(errors='replace')
        assert result.returncode==1 and 'cannot prove index `32` is within length 32' in text,text
        results.append({'command':command,'exit':1,'refusal':'exact bounds obligation'})
    else:
        if options.route=='native':
            compile_command=[omega,'--target','linux_x86_64','--report-file',str(output/'native.compile.report'),source]
            result=execute(compile_command,b'',output,'native.compile',options.timeout)
            assert result.returncode==0,result.stderr.decode(errors='replace')
            publication=(output/'native.compile.report').read_text()
            paths=re.findall(r'^published native output to (.+)$',publication,re.M);assert len(paths)==1,publication
            command=[paths[0]]
        else:command=[omega,'run','--both',source]
        for name in oracle['cases']:
            result=execute(command,(ROOT/'data'/(name+'.txt')).read_bytes(),output,options.route+'.'+name,options.timeout)
            assert result.returncode==0,(name,result.returncode,result.stderr.decode(errors='replace'))
            check_report(result.stdout,name,oracle)
            if options.route=='both':
                text=result.stderr.decode(errors='replace');assert 'interp exit: 0' in text and 'DECLINED' not in text and 'DIVERGENCE' not in text,text
            results.append({'case':name,'command':command,'exit':0,'oracle':'all numerical values matched','engine_output':'native; CLI both compares termination only'})
        refusals=json.loads((ROOT/'validation'/'refusals.json').read_text())
        for name,expected in refusals.items():
            result=execute(command,(ROOT/'data'/(name+'.txt')).read_bytes(),output,options.route+'.'+name,options.timeout)
            assert result.returncode==expected['exit'],(name,result.returncode,result.stderr.decode(errors='replace'))
            assert expected['fragment'].encode() in result.stdout,(name,result.stdout)
            if options.route=='both':assert f"interp exit: {expected['exit']}" in result.stderr.decode(errors='replace')
            results.append({'case':name,'command':command,'exit':result.returncode,'refusal':expected['fragment']})
    (output/(options.route+'.results.json')).write_text(json.dumps(results,indent=2)+'\n')
    print(options.route+': '+str(len(results))+' outcomes matched')
if __name__=='__main__':main()
