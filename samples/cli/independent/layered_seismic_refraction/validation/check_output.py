"""Compare numerical values, branch identity, activation and deterministic ties."""
import hashlib
import json
import math
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
ORACLE_SHA256=(ROOT/'validation/oracle.sha256').read_text().strip()

def oracle():
    payload=(ROOT/'validation/oracle.json').read_bytes()
    assert hashlib.sha256(payload).hexdigest()==ORACLE_SHA256, 'frozen oracle changed'
    return json.loads(payload)

def check(name, output, code):
    expected=oracle()['cases'][name]
    assert hashlib.sha256((ROOT/'data'/f'{name}.stdin').read_bytes()).hexdigest()==expected['input_sha256']
    assert code==expected['exit'],(name,code,expected['exit'],output)
    lines=output.splitlines()
    if code:
        assert lines[-1:]==[expected['error']], (name,lines)
        return
    rows=[line.split() for line in lines]
    assert len(rows)==len(expected['rows']), (name,len(rows),len(expected['rows']),lines)
    for actual,wanted in zip(rows,expected['rows']):
        assert len(actual)==len(wanted),(name,actual,wanted)
        labels=2 if wanted[0]=='B' else 3
        assert actual[:labels]==[str(item) for item in wanted[:labels]],(name,actual,wanted)
        for observed,reference in zip(actual[labels:],wanted[labels:]):
            observed=float(observed);reference=float(reference)
            assert math.isfinite(observed)
            assert (observed==0.0 if reference==0.0 else abs(observed-reference)<=1e-7*abs(reference)),(name,observed,reference)

if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser();parser.add_argument('directory');parser.add_argument('--records',default='records.tsv')
    args=parser.parse_args();directory=Path(args.directory)
    records=dict(line.split('\t') for line in (directory/args.records).read_text().splitlines()[1:])
    assert set(records)==set(oracle()['cases'])
    for name,code in records.items():
        check(name,(directory/f'{name}.stdout').read_text(),int(code))
        assert not (directory/f'{name}.stderr').read_bytes(),name
    print(f'verified {len(records)} captured value records')
