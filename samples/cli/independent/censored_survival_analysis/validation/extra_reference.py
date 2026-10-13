"""Additional boundary cases frozen after review, before execution."""
from pathlib import Path
import hashlib,json
from fractions import Fraction as Q
from reference import analyze, ROOT
cases={
 'exact_budget':('1\n0 0 0\n'+' '*(4096-8),[(Q(0),0,0)]),
 'budget_with_record':('1\n0 0 0\n'+' '*(4097-8),None),
 'upper_time':('2\n1000000 1 0\n999999.999999 0 1\n',[(Q(1000000),1,0),(Q('999999.999999'),0,1)]),
 'six_decimals':('2\n0.000001 1 0\n0.000002 1 1\n',[(Q('0.000001'),1,0),(Q('0.000002'),1,1)]),
 'tied_mixed':('8\n'+''.join(f'1 {int(i<4)} {i%2}\n' for i in range(8)),[(Q(1),int(i<4),i%2) for i in range(8)]),
 'maximum_complete':('32\n'+''.join(f'{i} 1 {i%2}\n' for i in range(32)),[(Q(i),1,i%2) for i in range(32)]),
 'exact_half':('24\n'+''.join(f'{t} {e} 0\n' for t,e in [(0,1)]*4+[(1,1)]*7+[(2,1)]+[(2,0)]*5+[(3,0)]*7),[(Q(t),e,0) for t,e in [(0,1)]*4+[(1,1)]*7+[(2,1)]+[(2,0)]*5+[(3,0)]*7]),
 'fraction_time_missing':('1\n1. 1 0\n',None),
}
expected={};paths=[Path(__file__)]
for name,(wire,rows) in cases.items():
 directory=ROOT/'data'/name;directory.mkdir(exist_ok=True)
 directory.joinpath('input.txt').write_text(wire);directory.joinpath('expected_exit.txt').write_text('0\n' if rows is not None else '2\n')
 expected[name]=analyze(rows) if rows is not None else {'error':'ERROR invalid input\n'}
 paths+=list(directory.glob('*'))
p=ROOT/'validation/extra_expected.json';p.write_text(json.dumps(expected,indent=2)+'\n');paths.append(p)
p=ROOT/'validation/all_cases.txt';p.write_text((ROOT/'validation/cases.txt').read_text()+''.join(name+'\n' for name in cases));paths.append(p)
(ROOT/'validation/extra-freeze.sha256').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(ROOT))+'\n' for p in paths))
