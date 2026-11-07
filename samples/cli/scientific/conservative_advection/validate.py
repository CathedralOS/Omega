"""Validate captured numerical channels or fresh native processes against frozen oracles."""
import argparse, hashlib, json, math, subprocess
from pathlib import Path
from decimal import Decimal, localcontext
ROOT=Path(__file__).resolve().parent

def frozen():
    manifests=[ROOT/'data/freeze.sha256',ROOT/'data/supplemental_freeze.sha256',ROOT/'data/near_one_freeze.sha256']
    for line in '\n'.join(path.read_text() for path in manifests).splitlines():
        if not line.strip(): continue
        digest,path=line.split('  ',1)
        assert hashlib.sha256((ROOT/path).read_bytes()).hexdigest()==digest, f'changed frozen input: {path}'
    return json.loads((ROOT/'data/oracle.json').read_text())+json.loads((ROOT/'data/supplemental_oracle.json').read_text())+json.loads((ROOT/'data/near_one_oracle.json').read_text())

def inspect(case,code,stdout,stderr):
    assert code==case['exit'], (case['name'],code,case['exit'],stdout,stderr)
    assert not stderr, (case['name'],stderr)
    if code:
        assert stdout.startswith('REJECT ') and len(stdout.splitlines())==1, (case['name'],stdout)
        return
    lines=stdout.splitlines()
    assert lines[-1]=='ADVECTED' and len(lines)==len(case['initial'])+5
    values={}; field=[]
    for line in lines[:-1]:
        label,mantissa,exponent=line.split()
        value=Decimal(f'{mantissa}e{exponent}')
        assert value.is_finite()
        if label=='CELL': field.append(value)
        else:
            assert label not in values
            values[label]=value
    assert set(values)=={'MASS','MIN','MAX','CFL'}
    expected=case['expected']; initial=[Decimal(str(value)) for value in case['initial']]
    for actual,wanted in zip(field,map(Decimal,expected['field'])):
        assert abs(actual-wanted)<=max(Decimal('1e-9'),Decimal('1e-9')*abs(wanted)), (case['name'],'field',actual,wanted)
    # Mass tolerance uses the initial total and final total's natural scale;
    # no unit floor, so tiny concentration controls still test conservation.
    mass=Decimal(expected['mass']); scale=abs(mass)
    with localcontext() as context:
        context.prec=80
        assert abs(values['MASS']-mass)<=Decimal('1e-10')*scale, (case['name'],'reported mass',values['MASS'],mass)
        reconstructed=sum(field)*Decimal(str(case['spacing']))
        assert abs(reconstructed-mass)<=Decimal('1e-10')*scale, (case['name'],'field mass')
    for label,key in [('MIN','minimum'),('MAX','maximum'),('CFL','cfl')]:
        assert abs(values[label]-Decimal(expected[key]))<=max(Decimal('1e-9'),Decimal('1e-9')*abs(Decimal(expected[key]))), (case['name'],label)
    tolerance=Decimal('1e-12')*max(initial)
    assert min(field)>=0 and min(field)>=min(initial) and max(field)<=max(initial), (case['name'],'extrema')
    assert abs(values['MIN']-min(field))<=tolerance and abs(values['MAX']-max(field))<=tolerance
    if Decimal(expected['cfl']).copy_abs()==1 or Decimal(expected['cfl'])==0:
        assert field==list(map(Decimal,expected['field'])), (case['name'],'exact shift or identity')

def main():
    parser=argparse.ArgumentParser(); group=parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--native-binary',type=Path); group.add_argument('--captured',type=Path)
    group.add_argument('--omega',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'build/validation/native')
    arguments=parser.parse_args(); cases=frozen()
    if arguments.omega:
        arguments.output.mkdir(parents=True,exist_ok=True)
        for case in cases:
            assert (ROOT/'data'/(case['name']+'.stdin')).read_bytes()==case['stdin'].encode()
        result=subprocess.run([str(arguments.omega.resolve()),'run','--both','--keep',
            '--inputs',str(ROOT/'data'),'--outputs',str(arguments.output.resolve()),
            str(ROOT/'main.omg')],input=b'',capture_output=True,timeout=1800)
        (arguments.output/'invocation.stdout').write_bytes(result.stdout)
        (arguments.output/'invocation.stderr').write_bytes(result.stderr)
        (arguments.output/'invocation.exit').write_text(str(result.returncode)+'\n')
        assert result.returncode==0, result.stderr.decode()
        rows=[line.split('\t') for line in (arguments.output/'results.tsv').read_text().splitlines()[1:]]
        assert all(len(row)==4 for row in rows)
        records={row[0]:row for row in rows}
        assert len(records)==len(rows)==len(cases) and set(records)=={case['name'] for case in cases}
        for case in cases:
            name,native_exit,verdict,terminal_exit=records[case['name']]
            assert verdict=='agrees', records[name]
            for prefix,code in [('',native_exit),('terminal.',terminal_exit)]:
                inspect(case,int(code),(arguments.output/(name+'.'+prefix+'stdout')).read_text(),
                    (arguments.output/(name+'.'+prefix+'stderr')).read_text())
            print(name,'native and Terminal verified',native_exit,terminal_exit)
        print(f'{len(cases)} frozen cases verified in each engine; numerical field, mass, extrema and CFL checked')
        return
    records={}
    if arguments.captured:
        records=dict(line.split('\t') for line in (arguments.captured/'records.tsv').read_text().splitlines()[1:])
        assert set(records)=={case['name'] for case in cases},'captured matrix incomplete'
    else: arguments.output.mkdir(parents=True,exist_ok=True)
    for case in cases:
        if arguments.captured:
            code=int(records[case['name']]); stdout=(arguments.captured/(case['name']+'.stdout')).read_text(); stderr=(arguments.captured/(case['name']+'.stderr')).read_text()
        else:
            result=subprocess.run([str(arguments.native_binary.resolve())],input=case['stdin'].encode(),capture_output=True,timeout=15)
            code=result.returncode; stdout=result.stdout.decode(); stderr=result.stderr.decode()
            for suffix,payload in [('stdout',stdout),('stderr',stderr),('exit',str(code))]: (arguments.output/(case['name']+'.'+suffix)).write_text(payload)
        inspect(case,code,stdout,stderr)
        print(case['name'],'verified',code)
    print(f'{len(cases)} frozen cases verified; numerical field, mass, extrema and CFL checked')
if __name__=='__main__': main()
