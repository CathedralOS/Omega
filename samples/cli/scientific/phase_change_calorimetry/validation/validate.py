"""Validate actual numerical native and retained Terminal observations.
No package authority acceptance is performed here. Prepare/review first.
"""
import argparse,hashlib,json,math,re,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def frozen():
    manifest=json.loads((ROOT/'validation/FREEZE.json').read_text())
    for path,wanted in manifest.items():assert digest(ROOT/path)==wanted,('frozen identity changed',path)
    oracle=json.loads((ROOT/'validation/oracle.json').read_text())
    supplement=json.loads((ROOT/'validation/SUPPLEMENT_FREEZE.json').read_text())
    for path,wanted in supplement.items():assert digest(ROOT/path)==wanted,('supplement identity changed',path)
    oracle['cases'].update(json.loads((ROOT/'validation/supplement_oracle.json').read_text())['cases'])
    return oracle
def check(case,text,engine):
    lines=text.splitlines();assert lines[-1]=='status=ok',(engine,lines)
    expected_keys=list(case['rows'][0]);assert len(lines)==8*len(case['rows'])+1
    parsed=[]
    for observation,expected in enumerate(case['rows']):
        pairs=[line.split('=') for line in lines[8*observation:8*observation+8]]
        assert [p[0] for p in pairs]==[key+'_pico' for key in expected_keys]
        assert all(re.fullmatch(r'-?[0-9]+',p[1]) for p in pairs)
        values={key:int(pair[1])/1e12 for key,pair in zip(expected_keys,pairs)}
        for key,wanted in expected.items():
            limit=1e-5 if key.startswith('temperature') else 1e-6*max(1,abs(wanted)) if key.startswith('enthalpy') or key=='transferred_energy' else 1e-8
            assert abs(values[key]-wanted)<=limit,(engine,observation,key,values[key],wanted,limit)
        assert 0<=values['fraction1']<=1 and 0<=values['fraction2']<=1
        parsed.append(values)
    if 'analytic_equilibrium' in case:
        for key,wanted in case['analytic_equilibrium'].items():
            assert abs(parsed[-1][key]-wanted)<=1e-8,(engine,'analytic partition',key,parsed[-1][key],wanted)
    return parsed

def refusals():
    good=(ROOT/'data/melting.txt').read_text().split()
    result={'empty':'','extra':' '.join(good)+' 9','byte_budget':' '*4096+' '.join(good),'invalid_byte':'?','missing':' '.join(good[:-1])}
    for position in (0,1,2,3,5,6,7,8):
        for value in ('0','-1','1001','nan','inf'):
            changed=good.copy();changed[position]=value;result[f'material_{position}_{value}']=' '.join(changed)
    for position,value in ((10,'-1'),(10,'1001'),(11,'9'),(11,'1.5'),(11,'-1'),(12,'-1'),(13,'-1'),(14,'0.01'),(15,'1001'),(0,'1e2'),(0,'1.'),(0,'.5'),(0,'0.1234567890')):
        changed=good.copy();changed[position]=value;result[f'option_{position}_{value}']=' '.join(changed)
    return result

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--omega',required=True,type=Path);parser.add_argument('--route',choices=('all','check','native','terminal','invalid'),default='all');parser.add_argument('--runner',type=Path);parser.add_argument('--output',type=Path,default=ROOT/'build/qualification')
    options=parser.parse_args();output=options.output.resolve();output.mkdir(parents=True,exist_ok=True);omega=options.omega.resolve();oracle=frozen()
    identities={str(p):digest(p) for p in [omega,*sorted(ROOT.rglob('*.omg'))] if 'build' not in p.relative_to(ROOT).parts} if omega.is_relative_to(ROOT) else {str(omega):digest(omega),**{str(p):digest(p) for p in sorted(ROOT.rglob('*.omg')) if 'build' not in p.relative_to(ROOT).parts}}
    results=[]
    def execute(name,command,expected,fragment=None):
        started=time.monotonic();run=subprocess.run(list(map(str,command)),input=b'',capture_output=True,timeout=600)
        (output/(name+'.stdout')).write_bytes(run.stdout);(output/(name+'.stderr')).write_bytes(run.stderr)
        results.append(dict(name=name,command=list(map(str,command)),exit=run.returncode,seconds=time.monotonic()-started));assert run.returncode==expected,(name,run.stdout,run.stderr)
        if fragment:assert fragment.encode() in run.stdout+run.stderr,(name,run.stdout,run.stderr)
        return run
    if options.route in ('all','check'):execute('check',[omega,'--check','--target','linux_x86_64',ROOT/'main.omg'],0)
    if options.route in ('all','invalid'):execute('source_invalid',[omega,'--check','--target','linux_x86_64',ROOT/'controls/observation/main.omg'],1,'cannot prove index `8` is within length 8')
    if options.route in ('all','native','terminal'):
        runner=options.runner.resolve() if options.runner else omega.parent/'phase-change-validation'
        assert runner.is_file(),f'build public SDK numerical runner first: {runner}'
        cases={name:(ROOT/'data'/(name+'.txt')).read_text() for name in oracle['cases']};cases.update(refusals());cases['zero_observations']='1 1 1 1 0 1 1 1 1 0 0 0\n';cases['repeated_times']='1 1 1 1 0 1 1 1 1 0 1 3 0 0 0\n'
        inputs=[]
        for name,text in cases.items():
            directory=output/name;directory.mkdir(exist_ok=True);path=directory/'input.txt';path.write_text(text);inputs.append(path)
        execute('capture',[runner,ROOT/'main.omg',output,*inputs],0)
        for name in cases:
            for engine in ('native','terminal') if options.route=='all' else (options.route,):
                directory=output/name;text=(directory/(engine+'.stdout')).read_text();assert (directory/(engine+'.stderr')).read_bytes()==b''
                code=0 if name in oracle['cases'] or name in ('zero_observations','repeated_times') else 64
                status=(directory/(engine+'.status')).read_text()
                status_matches = status==str(code) if engine=='native' else (code==0 and status=='Complete(Unit)') or re.search(r'return_code: '+str(code)+r'\s*[,}]',status)
                assert status_matches,(name,engine,status)
                if name in oracle['cases']:
                    rows=check(oracle['cases'][name],text,engine);v=list(map(float,cases[name].split()));energy=v[0]*v[4]+v[5]*v[9];scale=oracle['cases'][name]['energy_scale']
                    for row in rows:
                        assert abs(v[0]*row['enthalpy1']+v[5]*row['enthalpy2']-energy)<=1e-10*scale,(name,engine,'conservation')
                        assert abs(row['transferred_energy']-v[0]*(v[4]-row['enthalpy1']))<=1e-10*scale
                    print(name,engine,'numerical PASS',flush=True)
                elif name=='zero_observations':assert text=='status=ok\n'
                elif name=='repeated_times':assert len(text.splitlines())==25 and text.endswith('status=ok\n')
                else:assert text=='INVALID calorimetry input\n',(name,engine,text)
                results.append(dict(name=name,engine=engine,exit=code,stdout_sha256=hashlib.sha256(text.encode()).hexdigest(),numerical=name in oracle['cases']))
    assert all(digest(Path(path))==value for path,value in identities.items()),'inputs changed during qualification'
    (output/'results.json').write_text(json.dumps(dict(route=options.route,producer_sha256=digest(omega),source_hashes=identities,oracle_sha256=digest(ROOT/'validation/oracle.json'),results=results),indent=2)+'\n')
    print('PASS',len(results),'outcomes')
if __name__=='__main__':main()
