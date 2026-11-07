"""Check immutable reference and real native/captured Terminal numeric output."""
import argparse, hashlib, json, math, subprocess
from pathlib import Path
from reference import reference
ROOT=Path(__file__).resolve().parents[1]
ORACLE_HASH='92d5871f34cb91e4035ef2a4abdb3a586d66e67d6a4ca2a4cbeafb218f91b9d6'

def close(actual,expected,scale=1):
    assert math.isfinite(actual), f'nonfinite observed {actual}'
    tolerance=1e-7*abs(expected)+2e-12*max(1,scale)
    assert abs(actual-expected)<=tolerance, f'{actual} != {expected}; tolerance {tolerance}'

def audit(oracle):
    for name,case in oracle['cases'].items():
        if case['exit']: continue
        observed=reference(case['nodes'],case['bars'])
        assert observed==case['expected'],f'{name}: independent reference differs'
        for key,expected in (case.get('analytic') or {}).items():
            actual=observed[key]
            if isinstance(expected,list):
                assert len(actual)==len(expected)
                for a,b in zip(actual,expected):close(a,b)
            else:close(actual,expected)
    print('frozen independent dense and analytic oracles certified')

def validate(name,case,code,stdout,stderr):
    assert not stderr, f'{name}: unexpected stderr {stderr[:200]}'
    assert code==case['exit'],f'{name}: exit {code}, expected {case["exit"]}'
    lines=stdout.decode('ascii').splitlines()
    if code:
        expected='MECHANISM:' if code==65 else 'INVALID:'
        assert len(lines)==1 and lines[0].startswith(expected),f'{name}: wrong refusal {lines}'
        return {}
    assert lines[0]=='planar_truss_elasticity; numeric values in consistent input units'
    nodes=case['nodes'];bars=case['bars'];expected=case['expected']
    assert len(lines)==2+len(nodes)+len(bars),f'{name}: wrong row count'
    displacement=[];reaction=[];extension=[];force=[]
    for i,node in enumerate(nodes):
        row=lines[1+i].split(',');assert len(row)==6 and row[0]=='N' and float(row[1])==i
        values=list(map(float,row[2:]));assert all(math.isfinite(x) for x in values)
        displacement+=values[:2];reaction+=values[2:]
        for axis in range(2):
            if node[2+axis]: assert values[axis]==0, f'{name}: support moved'
            else: assert values[2+axis]==0, f'{name}: reaction on free coordinate'
    for i,bar in enumerate(bars):
        row=lines[1+len(nodes)+i].split(',');assert len(row)==4 and row[0]=='B' and float(row[1])==i
        extension.append(float(row[2]));force.append(float(row[3]))
    row=lines[-1].split(',');assert len(row)==7 and row[0]=='S'
    energy,work,residual,balance_x,balance_y,moment=map(float,row[1:]);assert all(math.isfinite(x) for x in [energy,work,residual,balance_x,balance_y,moment])
    for key,actual in [('displacement',displacement),('reaction',reaction),('extension',extension),('force',force)]:
        assert len(actual)==len(expected[key]);scale=max([abs(x) for x in expected[key]]+[1e-12])
        for a,b in zip(actual,expected[key]): close(a,b,scale)
    close(energy,expected['energy']);close(work,expected['work'])
    # Independently reconstruct equilibrium from emitted bar forces, rather than
    # trusting the application residual report or native/Terminal exit parity.
    internal=[0.0]*(2*len(nodes))
    for (a,b,ea),ext,f in zip(bars,extension,force):
        dx=nodes[b][0]-nodes[a][0];dy=nodes[b][1]-nodes[a][1];length=math.hypot(dx,dy)
        measured=(dx*(displacement[2*b]-displacement[2*a])+dy*(displacement[2*b+1]-displacement[2*a+1]))/length
        # Four printed endpoint coordinates contribute at most one picounit
        # each. Propagate that emission uncertainty through the axial law;
        # comparisons against the frozen independent force oracle stay strict.
        extension_emission = 2e-12 * (abs(dx) + abs(dy)) / length
        assert abs(ext-measured) <= 1e-7*abs(measured) + 1e-12 + extension_emission
        force_emission = 1e-12 + ea / length * extension_emission
        assert abs(f-ea*measured/length) <= 1e-7*abs(ea*measured/length) + force_emission
        for axis,direction in enumerate([dx/length,dy/length]):
            internal[2*a+axis]-=f*direction;internal[2*b+axis]+=f*direction
    loads=[n[4+axis] for n in nodes for axis in range(2)]
    load_scale=max([abs(x) for x in loads]+[1.0]);limit=1e-8*load_scale
    actual_residual=max(abs(a-b-c) for a,b,c in zip(internal,loads,reaction))
    assert 0<=residual<=limit and actual_residual<=limit,f'{name}: equilibrium {residual}, reconstructed {actual_residual}, limit {limit}'
    totals=[loads[i]+reaction[i] for i in range(len(loads))]
    observed_x=sum(totals[::2]);observed_y=sum(totals[1::2]);observed_moment=sum(n[0]*totals[2*i+1]-n[1]*totals[2*i] for i,n in enumerate(nodes))
    coordinate_scale=max([abs(c) for n in nodes for c in n[:2]]+[1])
    for a,b in [(balance_x,observed_x),(balance_y,observed_y)]:
        assert abs(a)<=limit and abs(b)<=limit
    assert abs(moment)<=limit*coordinate_scale and abs(observed_moment)<=limit*coordinate_scale
    measured_work=sum(a*b for a,b in zip(loads,displacement));measured_energy=sum(a*b/2 for a,b in zip(force,extension))
    close(work,measured_work,max(abs(work),1));close(energy,measured_energy,max(abs(energy),1));assert abs(2*energy-work)<=1e-8*max(1,abs(work),abs(2*energy)), f'{name}: energy/work identity defect {abs(2*energy-work)}'
    return {'maximum_reconstructed_equilibrium_residual':actual_residual,'load_scale':load_scale,'energy_work_defect':abs(2*energy-work)}

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--oracle-only',action='store_true');parser.add_argument('--native',type=Path);parser.add_argument('--captured',type=Path);parser.add_argument('--output',type=Path,default=ROOT/'build/native-validation');options=parser.parse_args()
    path=ROOT/'validation/oracle.json';assert hashlib.sha256(path.read_bytes()).hexdigest()==ORACLE_HASH,'frozen oracle changed'
    oracle=json.loads(path.read_text());audit(oracle)
    if options.oracle_only:return
    assert bool(options.native)!=bool(options.captured),'choose --native or --captured'
    options.output.mkdir(parents=True,exist_ok=True);results={}
    for name,case in oracle['cases'].items():
        if options.native:
            completed=subprocess.run([str(options.native.resolve())],input=(ROOT/'data'/f'{name}.txt').read_bytes(),capture_output=True,timeout=20)
            code,stdout,stderr=completed.returncode,completed.stdout,completed.stderr
            for suffix,content in [('stdout',stdout),('stderr',stderr),('exit',f'{code}\n'.encode())]:(options.output/f'{name}.{suffix}').write_bytes(content)
        else:
            code=int((options.captured/f'{name}.exit').read_text());stdout=(options.captured/f'{name}.stdout').read_bytes();stderr=(options.captured/f'{name}.stderr').read_bytes()
        results[name]={'exit':code,**validate(name,case,code,stdout,stderr)}
        print(f'{name}: numeric/refusal qualification passed')
    (options.output/'results.json').write_text(json.dumps({'oracle_sha256':ORACLE_HASH,'route':'native' if options.native else 'captured Terminal','cases':results},indent=2)+'\n')
if __name__=='__main__': main()
