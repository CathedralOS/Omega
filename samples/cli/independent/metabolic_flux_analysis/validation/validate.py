#!/usr/bin/env python3
"""Check actual native and retained Terminal numbers against frozen LP oracles."""
import argparse, hashlib, json, math, os, pathlib, re, shutil, subprocess
from fractions import Fraction
from reference import oracle, scipy

ROOT = pathlib.Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_output(raw, case):
    text=raw.decode('ascii')
    if case['exit'] != 0:
        assert text == ('INFEASIBLE\n' if case['exit']==11 else 'ERROR input\n'), repr(text)
        return
    lines=text.splitlines()
    assert lines.pop(0)=='OPTIMAL'
    observed={name:[] for name in ('objective_pico','flux_pico','minimum_pico','maximum_pico','residual_pico')}
    for line in lines:
        match=re.fullmatch(r'([a-z_]+)=(-?[0-9]+)',line)
        assert match and match[1] in observed, line
        observed[match[1]].append(int(match[2])/1e12)
    count=len(case['bounds']);metabolites=len(case['matrix'])
    assert list(map(len,observed.values())) == [1,count,count,count,metabolites], observed
    values=observed['flux_pico']; expected=case['oracle']
    assert abs(observed['objective_pico'][0]-float(Fraction(expected['optimum']))) <= 1e-7
    assert abs(values[case['biomass']]-observed['objective_pico'][0]) <= 1e-8
    for value,(lower,upper) in zip(values,case['bounds']):
        low=float(Fraction(lower));high=float(Fraction(upper))
        assert (low-value)/max(1,abs(low),abs(value)) <= 1e-8
        assert (value-high)/max(1,abs(high),abs(value)) <= 1e-8
    for row,reported in zip(case['matrix'],observed['residual_pico']):
        products=[coefficient*value for coefficient,value in zip(row,values)]
        residual=math.fsum(products);scale=max(1,math.fsum(abs(value) for value in products))
        assert abs(residual)/scale <= 1e-8
        assert abs(reported-residual)/scale <= 1e-8
    for name,expected_name in (('minimum_pico','minima'),('maximum_pico','maxima')):
        for observed_value,expected_value in zip(observed[name],expected[expected_name]):
            assert abs(observed_value-float(Fraction(expected_value))) <= 1e-7, (name,observed_value,expected_value)
    # Order and closure are meaningful; a degenerate optimum vector is not
    # required to match the independent solver's arbitrary chosen vector.
    for low,value,high in zip(observed['minimum_pico'],values,observed['maximum_pico']):
        assert low-1e-8 <= value <= high+1e-8


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--omega',required=True)
    parser.add_argument('--route',choices=('native','terminal','all'),default='all')
    parser.add_argument('--case',action='append')
    arguments=parser.parse_args()
    binary=pathlib.Path(arguments.omega).resolve()
    manifest=json.loads((ROOT/'validation/frozen_oracle.json').read_text())
    assert digest(ROOT/'validation/frozen_oracle.json')=='35bd4159ec9627da101c176985936a0bbcf3fe39c46736c533bbc62e6a37b33c'
    assert scipy.__version__ == manifest['scipy_version']
    evidence=ROOT/'build/validation'/arguments.route;evidence.mkdir(parents=True,exist_ok=True)
    metadata=dict(compiler=str(binary),compiler_sha256=digest(binary),revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),route=arguments.route,complete=False,scipy_version=scipy.__version__,target='linux_x86_64',oracle_sha256=digest(ROOT/'validation/frozen_oracle.json'),sources={str(path.relative_to(ROOT)):digest(path) for path in sorted(ROOT.rglob('*.omg')) if 'build' not in path.relative_to(ROOT).parts},cases=[])
    (evidence/'results.json').write_text(json.dumps(metadata,indent=2)+'\n')
    try:
        update=subprocess.run([str(binary),'update','--project',str(ROOT),'--target','linux_x86_64'],stdin=subprocess.DEVNULL,capture_output=True,timeout=240)
    except subprocess.TimeoutExpired as error:
        (evidence/'update.stdout').write_bytes(error.stdout or b'')
        (evidence/'update.stderr').write_bytes(error.stderr or b'')
        metadata['update_timeout_seconds']=240
        (evidence/'results.json').write_text(json.dumps(metadata,indent=2)+'\n')
        raise
    (evidence/'update.stdout').write_bytes(update.stdout);(evidence/'update.stderr').write_bytes(update.stderr)
    metadata['update_exit']=update.returncode
    (evidence/'results.json').write_text(json.dumps(metadata,indent=2)+'\n')
    assert update.returncode==0,update.stderr.decode()
    for case in manifest['cases']:
        if arguments.case and case['name'] not in arguments.case:
            continue
        assert hashlib.sha256(case['input'].encode()).hexdigest()==case['sha256']
        if case['exit'] in (0,11):
            recomputed=oracle(case['matrix'],case['bounds'],case['biomass'])
            for key in ('status','optimum','minima','maxima','vertices','optimal_vertices'):
                assert recomputed.get(key)==case['oracle'].get(key),(case['name'],key)
        directory=evidence/case['name'];directory.mkdir(exist_ok=True)
        command=[str(binary),'run','--keep']
        if arguments.route in ('terminal','all'):
            command.append('--both')
        command.append(str(ROOT/'main.omg'))
        (directory/'input.txt').write_text(case['input'])
        try:
            result=subprocess.run(command,input=case['input'].encode(),capture_output=True,timeout=180)
        except subprocess.TimeoutExpired as error:
            (directory/'native.stdout').write_bytes(error.stdout or b'')
            (directory/'run.stderr').write_bytes(error.stderr or b'')
            metadata['cases'].append(dict(name=case['name'],command=command,timeout_seconds=180,native=False,terminal=False))
            (evidence/'results.json').write_text(json.dumps(metadata,indent=2)+'\n')
            raise
        (directory/'native.stdout').write_bytes(result.stdout);(directory/'run.stderr').write_bytes(result.stderr)
        row=dict(name=case['name'],command=command,exit=result.returncode,expected_exit=case['exit'],native=False,terminal=False)
        metadata['cases'].append(row)
        (evidence/'results.json').write_text(json.dumps(metadata,indent=2)+'\n')
        assert result.returncode==case['exit'],(case['name'],result.stderr.decode())
        if arguments.route in ('native','all'):
            validate_output(result.stdout,case);row['native']=True
        retained=re.search(rb'build dir kept: ([^\r\n]+)',result.stderr)
        assert retained,result.stderr
        product=pathlib.Path(os.fsdecode(retained[1]))
        try:
            if arguments.route in ('terminal','all'):
                assert f'interp exit: {case["exit"]}'.encode() in result.stderr and b'DECLINED' not in result.stderr,result.stderr.decode()
                terminal=(product/'terminal.stdout').read_bytes()
                (directory/'terminal.stdout').write_bytes(terminal)
                (directory/'terminal.stderr').write_bytes((product/'terminal.stderr').read_bytes())
                validate_output(terminal,case);row['terminal']=True
                assert not (product/'terminal.stderr').read_bytes()
            row['retained_product']=str(product)
        finally:
            shutil.rmtree(product)
        (evidence/'results.json').write_text(json.dumps(metadata,indent=2)+'\n')
        print(case['name'],'PASS',arguments.route,flush=True)
    control=ROOT/'controls/reaction_access/main.omg'
    rejected=subprocess.run([str(binary),'--check',str(control)],stdin=subprocess.DEVNULL,capture_output=True,timeout=90)
    (evidence/'source-invalid.stderr').write_bytes(rejected.stderr)
    assert rejected.returncode==1 and b'cannot prove index `6` is within length 6' in rejected.stderr,rejected.stderr.decode()
    metadata['source_invalid']=dict(exit=rejected.returncode,checked=True)
    metadata['complete']=True
    (evidence/'results.json').write_text(json.dumps(metadata,indent=2)+'\n')
    print(len(metadata['cases']),'cases and source-invalid control validated',flush=True)

if __name__=='__main__':
    main()
