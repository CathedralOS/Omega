import json, math
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]

def check(name,text,exit_code):
    oracle=json.loads((ROOT/'validation/oracle.json').read_text())[name]
    assert exit_code==oracle['exit'], (name,exit_code,oracle['exit'],text)
    if exit_code:
        assert text.strip()==oracle['message'], (name,text)
        return
    lines=text.splitlines(); assert len(lines)==14,(name,text)
    for i,expected in enumerate(oracle['factors']):
        parts=lines[i].split(','); assert len(parts)==3; assert parts[:2]==['factor',str(i)]
        assert math.isclose(float(parts[2]),expected,rel_tol=2e-8,abs_tol=2e-8),(name,i,parts,expected)
    for i,row in enumerate(oracle['rows']):
        parts=lines[i+6].split(','); assert parts[:3]==['gene',str(i),row['reason']],(name,i,parts,row)
        if row['reason']!='tested': assert len(parts)==3; continue
        assert len(parts)==12
        for column,(actual,expected) in enumerate(zip(map(float,parts[3:]),row['values'])):
            # normal-tail approximation has an absolute error guarantee.
            absolute=4e-7 if column>=7 else 2e-5 if column==6 else 2e-7
            assert math.isclose(actual,expected,rel_tol=2e-7,abs_tol=absolute),(name,i,column,actual,expected)

if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser(); p.add_argument('directory',type=Path); args=p.parse_args()
    records=(args.directory/'records.tsv').read_text().splitlines(); assert records[0]=='case\texit'
    exits=dict(line.split('\t') for line in records[1:])
    expected=json.loads((ROOT/'validation/oracle.json').read_text())
    assert len(records)==len(expected)+1 and set(exits)==set(expected), 'incomplete Terminal observations'
    for name,exit_code in exits.items(): check(name,(args.directory/(name+'.stdout')).read_text(),int(exit_code)); print('verified',name)
