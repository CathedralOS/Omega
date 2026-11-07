"""Validate already captured executions; never modifies the frozen oracle."""
import json, math, sys
from pathlib import Path
root=Path(__file__).resolve().parent
expected=json.loads((root/'expected.json').read_text())
folder=Path(sys.argv[1]); route=sys.argv[2]
inventory=[line.split('\t') for line in (root/'input_cases.tsv').read_text().splitlines() if not line.startswith('#')]
assert len(inventory)==25 and len({case for case,_,_ in inventory})==25
for case,encoded,key in inventory:
    assert key in expected['cases'] or key in expected['rejections'] or key=='invalid_input'
    assert (folder/f'{route}-{case}.stdin').read_bytes()==bytes.fromhex(encoded),(case,'wrong captured input')
for case,_,key in inventory:
    if key not in expected['cases']:continue
    oracle=expected['cases'][key]
    text=(folder/f'{route}-{case}.stdout').read_text()
    code=int((folder/f'{route}-{case}.exit').read_text())
    assert code==oracle['exit'],(case,code,text)
    rows=[]; works=[]; balance=None
    for line in text.splitlines():
        fields=line.split(',')
        if len(fields)==4 and fields[0].isdigit():rows.append(list(map(float,fields)))
        if fields[0]=='work':
            assert int(fields[1])==len(works)
            works.append(float(fields[2]))
        if fields[0]=='balance':balance=list(map(float,fields[1:]))
    assert len(rows)==len(oracle['rows']) and len(works)==len(oracle['works']),case
    for actual,wanted in zip(rows,oracle['rows']):
        assert actual[:2]==wanted[:2]
        for a,b in zip(actual[2:],wanted[2:]):
            assert math.isfinite(a) and abs(a-b)<=expected['printed_absolute_tolerance'],(case,a,b)
    for actual,wanted in [(works,oracle['works']),(balance,oracle['balance'])]:
        assert actual is not None and len(actual)==len(wanted)
        assert all(math.isfinite(a) and abs(a-b)<=expected['printed_absolute_tolerance'] for a,b in zip(actual,wanted)),(case,actual,wanted)
    assert text.splitlines()[-1]==oracle['closure'],case
for case,_,key in inventory:
    if key not in expected['rejections']:continue
    code,message=expected['rejections'][key]
    actual=int((folder/f'{route}-{case}.exit').read_text())
    text=(folder/f'{route}-{case}.stdout').read_text()
    assert actual==code and text.splitlines()[-1]==message,(case,actual,text)
for case,_,key in inventory:
    if key!='invalid_input':continue
    assert int((folder/f'{route}-{case}.exit').read_text())==64
    assert (folder/f'{route}-{case}.stdout').read_text().strip()=='invalid_input'
print(f'{route}: 14 analytic cases, 6 invalid stages, 5 invalid inputs verified')
