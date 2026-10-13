"""Qualify real native and retained Terminal numerical outputs against a freeze.

The Terminal sponsor links the supplied CLI's exact recorded library closure;
normal complete-project preparation and runtime authority stay enforced.
"""
import argparse, hashlib, json, math, subprocess, sys, time
sys.dont_write_bytecode=True
from decimal import Decimal
from fractions import Fraction
from pathlib import Path
from reference import cdf
ROOT=Path(__file__).resolve().parents[1]
REPOSITORY=ROOT.parents[3]

def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def load():
    frozen=json.loads((ROOT/'validation/FREEZE.json').read_text())
    for name in ('reference','oracle'):
        extension='py' if name=='reference' else 'json'
        assert digest(ROOT/f'validation/{name}.{extension}')==frozen[name+'_sha256'],name+' changed since freeze'
    oracle=json.loads((ROOT/'validation/oracle.json').read_text())
    for name,case in oracle['cases'].items():
        assert digest(ROOT/'data'/f'{name}.txt')==case['input_sha256'],name+' input changed'
    return oracle

def check(stdout,stderr,code,name,oracle):
    case=oracle['cases'][name]
    assert stderr==b'',(name,stderr)
    if 'exit' in case:
        assert code==case['exit'] and stdout==case['stdout'].encode(),(name,code,stdout)
        return {'case':name,'exit':code,'oracle':'exact refusal matched'}
    assert code==0,(name,code,stdout)
    lines=stdout.decode('ascii').splitlines()
    assert len(lines)==7*len(case['rows'])+2*(case['future']+1)+1 and lines[-1]=='status=ok',(name,lines)
    position=0;rows=[]
    def item(key):
        nonlocal position
        pair=lines[position].split('=');position+=1
        assert len(pair)==2 and pair[0]==key,(name,pair,key)
        return int(pair[1])
    for wanted in case['rows']:
        alpha,beta=item('alpha'),item('beta');assert (alpha,beta)==(wanted['alpha'],wanted['beta'])
        values={}
        for key in ('mean','variance','lower','upper','above_half'):
            observed=Decimal(item(key+'_pico'))/Decimal(10**12);values[key]=observed
            limit=Decimal('1e-7') if key in ('lower','upper') else Decimal('1e-10')
            assert abs(observed-Decimal(wanted[key]))<=limit,(name,key,observed,wanted[key])
            assert 0<=observed<=1,(name,key,observed)
        for key,target in [('lower',Decimal('.025')),('upper',Decimal('.975'))]:
            residual=abs(cdf(alpha,beta,values[key])-target)
            assert residual<=Decimal('1e-8'),(name,key,'CDF residual',str(residual))
        assert values['lower']<=values['upper']
        rows.append(values)
    masses=[]
    for bin_index,wanted in enumerate(case['predictive']):
        assert item('bin')==bin_index
        observed=Decimal(item('probability_pico'))/Decimal(10**12)
        exact=Decimal(wanted['numerator'])/Decimal(wanted['denominator'])
        assert 0<=observed<=1 and abs(observed-exact)<=Decimal('1e-9'),(name,bin_index,observed,exact)
        masses.append(observed)
    assert abs(sum(masses)-1)<=Decimal('5e-11'),(name,'normalization',sum(masses))
    expected_mean=Decimal(case['future']*case['rows'][-1]['alpha'])/Decimal(case['rows'][-1]['alpha']+case['rows'][-1]['beta'])
    assert abs(sum(index*mass for index,mass in enumerate(masses))-expected_mean)<=Decimal('1e-8')
    return {'case':name,'exit':code,'oracle':'all numerical values and independent CDF residuals matched',
            'rows':[{key:str(value) for key,value in row.items()} for row in rows], 'predictive':[str(value) for value in masses]}

def invariants(results):
    valid={record['case']:record for record in results if 'rows' in record}
    for left,right in [('sequential','order'),('sequential','regroup'),('sequential','whitespace')]:
        if left in valid and right in valid:
            assert valid[left]['rows'][-1]==valid[right]['rows'][-1],(left,right,'final posterior')
            assert valid[left]['predictive']==valid[right]['predictive'],(left,right,'prediction')
    for left,right in [('all_success','all_failure'),('symmetry','symmetry_complement'),('skew_prior','skew_complement')]:
        if left in valid and right in valid:
            for first,second in zip(valid[left]['rows'],valid[right]['rows']):
                for a,b in [('mean','mean'),('lower','upper'),('upper','lower'),('above_half','above_half')]:
                    assert abs(Decimal(first[a])+Decimal(second[b])-1)<=Decimal('3e-12'),(left,right,a,b)
                assert abs(Decimal(first['variance'])-Decimal(second['variance']))<=Decimal('2e-12')
            assert all(abs(Decimal(a)-Decimal(b))<=Decimal('2e-12') for a,b in zip(valid[left]['predictive'],reversed(valid[right]['predictive'])))

def execute(command,stdin,output,name,timeout):
    result=subprocess.run(command,input=stdin,capture_output=True,timeout=timeout)
    for suffix,content in [('stdout',result.stdout),('stderr',result.stderr)]: (output/(name+'.'+suffix)).write_bytes(content)
    (output/(name+'.exit')).write_text(str(result.returncode)+'\n')
    return result

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--omega',required=True,type=Path)
    parser.add_argument('--route',choices=['all','native','terminal'],default='all')
    parser.add_argument('--output',type=Path,default=ROOT/'build/qualification')
    parser.add_argument('--timeout',type=int,default=180);parser.add_argument('--fuel',type=int,default=500_000_000)
    parser.add_argument('--case',action='append',dest='cases');options=parser.parse_args()
    assert options.timeout>0 and options.fuel>0
    output=options.output.resolve();output.mkdir(parents=True,exist_ok=True)
    omega=str(options.omega.resolve());oracle=load();names=options.cases or list(oracle['cases'])
    assert all(name in oracle['cases'] for name in names)
    started=time.monotonic();source=str(ROOT/'main.omg');record={'routes':{}}
    for name,path,code,fragment in [('check',ROOT/'main.omg',0,''),('source_invalid',ROOT/'controls/predictive_bin/main.omg',1,'cannot prove index `33` is within length 33')]:
        result=execute([omega,'--check',str(path)],b'',output,name,options.timeout)
        assert result.returncode==code and fragment.encode() in result.stdout+result.stderr,(name,result.stderr)
    if options.route in ('all','native'):
        report=output/'native.report'
        result=execute([omega,'--target','linux_x86_64','--report-file',str(report),source],b'',output,'compile',options.timeout)
        assert result.returncode==0,result.stderr
        paths=[line.removeprefix('published native output to ') for line in report.read_text().splitlines() if line.startswith('published native output to ')]
        assert len(paths)==1,report.read_text();program=paths[0];results=[]
        for name in names:
            observed=execute([program],(ROOT/'data'/f'{name}.txt').read_bytes(),output,name+'.native',options.timeout)
            results.append(check(observed.stdout,observed.stderr,observed.returncode,name,oracle))
        invariants(results);record['routes']['native']={'program_sha256':digest(Path(program)),'cases':results}
        print('Native:',len(results),'actual numerical/refusal outputs matched',flush=True)
    if options.route in ('all','terminal'):
        sponsor=output/'terminal-output'
        observed=execute([sys.executable,str(REPOSITORY/'tools/link_terminal_sponsor.py'),'--omega',omega,'--source',str(ROOT/'validation/terminal_output.rs'),'--output',str(sponsor)],b'',output,'sponsor_build',options.timeout)
        assert observed.returncode==0,observed.stderr.decode(errors='replace')
        observed=execute([str(sponsor),str(ROOT),str(output),str(options.fuel),str(options.timeout),*names],b'',output,'terminal_observer',options.timeout*len(names)+180)
        assert observed.returncode==0,observed.stdout+observed.stderr
        results=[]
        for name in names:
            results.append(check((output/f'{name}.terminal.stdout').read_bytes(),(output/f'{name}.terminal.stderr').read_bytes(),int((output/f'{name}.terminal.exit').read_text()),name,oracle))
        invariants(results);record['routes']['terminal']={'cases':results,'observer_build':json.loads(sponsor.with_suffix('.build.json').read_text()),'fuel_per_case':options.fuel}
        print('Terminal:',len(results),'actual retained numerical/refusal outputs matched',flush=True)
    record.update({'producer_version':subprocess.check_output([omega,'--version'],text=True).strip(),
        'producer_sha256':digest(Path(omega)),'source_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPOSITORY,text=True).strip(),
        'source_hashes':{str(path.relative_to(ROOT)):digest(path) for path in ROOT.rglob('*.omg') if 'build' not in path.relative_to(ROOT).parts},
        'freeze':json.loads((ROOT/'validation/FREEZE.json').read_text()),'elapsed_seconds':time.monotonic()-started,'target':'linux_x86_64'})
    (output/'results.json').write_text(json.dumps(record,indent=2)+'\n')
if __name__=='__main__':main()
