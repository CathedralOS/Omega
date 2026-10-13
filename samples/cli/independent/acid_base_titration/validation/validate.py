"""Compare actual numeric observations with the independently frozen roots."""
import argparse,hashlib,json,math,re,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
ENDPOINT_SHA256='db4504782ddca0829429af7a2e72984bad388bca732e69664996a1792d6f85f0'
ORACLE_SHA256='7faa6346e926f0052368bf1d2c971abeb6df0f2858220f0ac5d7e08da6b50456'
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def load_oracle():
    assert digest(ROOT/'validation/oracle.json')==ORACLE_SHA256
    frozen=json.loads((ROOT/'validation/FREEZE.json').read_text())
    assert digest(ROOT/'validation/reference.py')==frozen['reference_sha256']
    oracle=json.loads((ROOT/'validation/oracle.json').read_text())
    assert digest(ROOT/'validation/endpoint_oracle.json')==ENDPOINT_SHA256
    oracle['cases'].update(json.loads((ROOT/'validation/endpoint_oracle.json').read_text())['cases'])
    for name,case in oracle['cases'].items():assert digest(ROOT/'data'/(name+'.txt'))==case['input_sha256']
    return oracle

def check_report(stdout,name,oracle):
    lines=stdout.decode('ascii').splitlines();assert lines[-1]=='status=ok',(name,lines)
    keys=list(oracle['cases'][name]['rows'][0]);assert len(lines)==len(keys)*len(oracle['cases'][name]['rows'])+1
    rows=[]
    for position,expected in enumerate(oracle['cases'][name]['rows']):
        pairs=[line.split('=',1) for line in lines[position*len(keys):(position+1)*len(keys)]]
        assert [pair[0] for pair in pairs]==keys,(name,pairs)
        values={key:float(value) for key,value in pairs};assert all(map(math.isfinite,values.values()))
        acid=float(expected['total_acid']);charge_scale=max(acid,float(expected['counterion']),float(expected['H']),float(expected['OH']))
        for key,value in values.items():
            wanted=float(expected[key])
            if key=='pH':limit=1e-6
            elif key in ('mass_closure','H2A','HA','A','total_acid'):limit=1e-8*acid
            elif key=='charge_residual':limit=1e-8*charge_scale
            else:limit=1e-8*max(abs(wanted),1e-30)
            assert abs(value-wanted)<=limit,(name,position,key,value,wanted,limit)
        assert all(values[key]>=0 for key in ('H','OH','H2A','HA','A'))
        assert abs(values['H2A']+values['HA']+values['A']-values['total_acid'])<=1e-8*acid
        assert abs(values['H']+values['counterion']-values['OH']-values['HA']-2*values['A'])<=1e-8*charge_scale
        rows.append(values)
    return rows

def execute(command,stdin,output,name,timeout):
    result=subprocess.run(command,input=stdin,capture_output=True,timeout=timeout)
    for suffix,content in [('stdout',result.stdout),('stderr',result.stderr)]: (output/(name+'.'+suffix)).write_bytes(content)
    (output/(name+'.exit')).write_text(str(result.returncode)+'\n')
    return result

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--omega',required=True,type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'build/qualification');parser.add_argument('--timeout',type=int,default=300)
    options=parser.parse_args();output=options.output.resolve();output.mkdir(parents=True,exist_ok=True)
    omega=str(options.omega.resolve());oracle=load_oracle();source=str(ROOT/'main.omg');results=[]
    for name,command,code,fragment in [('check',[omega,'--check',source],0,''),('source_invalid',[omega,'--check',str(ROOT/'invalid/main.omg')],1,'cannot prove index `3` is within length 3')]:
        result=execute(command,b'',output,name,options.timeout);assert result.returncode==code,(name,result.stderr)
        assert fragment.encode() in result.stdout+result.stderr,(name,result.stderr)
        results.append({'case':name,'command':command,'exit':code,'fragment':fragment})
    command=[omega,'--target','linux_x86_64','--report-file',str(output/'native.report'),source]
    result=execute(command,b'',output,'compile',options.timeout);assert result.returncode==0,result.stderr
    publication=(output/'native.report').read_text();paths=re.findall(r'^published native output to (.+)$',publication,re.M);assert len(paths)==1,publication
    program=paths[0];all_rows={}
    refusals=json.loads((ROOT/'validation/refusals.json').read_text())
    for name in [*oracle['cases'],*refusals]:
        result=execute([program],(ROOT/'data'/(name+'.txt')).read_bytes(),output,name,options.timeout)
        assert result.stderr==b'',(name,result.stderr)
        if name in oracle['cases']:
            assert result.returncode==0,(name,result.returncode,result.stdout);all_rows[name]=check_report(result.stdout,name,oracle)
        else:assert result.returncode==refusals[name]['exit'] and refusals[name]['fragment'].encode() in result.stdout,(name,result.returncode,result.stdout)
        results.append({'case':name,'exit':result.returncode,'input_sha256':digest(ROOT/'data'/(name+'.txt')),'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'oracle':'matched'})
    for original,scaled in zip(all_rows['curve'],all_rows['volume_scaled']):
        for key in ('pH','H','OH','H2A','HA','A','total_acid','counterion'):assert abs(original[key]-scaled[key])<=1e-12*max(abs(original[key]),1e-30)
    record={'producer_version':subprocess.check_output([omega,'--version'],text=True).strip(),'producer_sha256':digest(Path(omega)),'oracle_sha256':ORACLE_SHA256,'source_hashes':{str(path.relative_to(ROOT)):digest(path) for path in ROOT.rglob('*.omg')},'results':results,'native_program_sha256':digest(Path(program))}
    (output/'results.json').write_text(json.dumps(record,indent=2)+'\n');print('Native:',len(results),'numerical and refusal outcomes matched')
if __name__=='__main__':main()
