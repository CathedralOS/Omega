"""Validate retained output only; this never generates expected physics values."""
import json, math, re, sys
from pathlib import Path
root=Path(__file__).parent
oracle=json.loads((root/'data/expected.json').read_text())
tolerance=oracle['output_absolute_tolerance']
def close(actual,expected):
    assert len(actual)==len(expected),(actual,expected)
    assert all(math.isfinite(a) and abs(a-e)<=tolerance for a,e in zip(actual,expected)),(actual,expected)
def validate(case,path,exit_code):
    expected=oracle['cases'][case]
    assert exit_code==expected['exit'],(case,exit_code,expected['exit'])
    text=Path(path).read_text()
    if exit_code==2:
        assert text=='invalid model: bounded finite disks, no overlap, restitution 0..1 required\n',text
        return
    contacts=[];ledgers=[];disks=[]
    for line in text.splitlines():
        if line.startswith('contact '):
            values=[float(x) for x in re.findall(r'-?\d+(?:\.\d+)?',line)]
            assert abs(values[-1])<=tolerance,line
            contacts.append(values[:-1])
        elif line.startswith('ledger '):
            ledgers.append([float(x) for x in re.findall(r'-?\d+(?:\.\d+)?',line)])
        elif line.startswith('disk '):
            values=[float(x) for x in re.findall(r'-?\d+(?:\.\d+)?',line)]
            assert values[0]==len(disks),line
            disks.append(values[1:])
    assert len(contacts)==len(expected['contacts']),(case,contacts)
    for actual,wanted in zip(contacts,expected['contacts']):close(actual,wanted)
    assert len(ledgers)==len(contacts)+(2 if exit_code==0 else 1),ledgers
    close(ledgers[-1],expected['ledger'])
    initial=ledgers[0]
    last_time=-1
    for time,px,py,energy,loss in ledgers:
        assert time>=last_time,(case,ledgers)
        last_time=time
        close([px,py,energy+loss],[initial[1],initial[2],initial[3]])
    assert len(disks)==len(expected['disks']),disks
    for actual,wanted in zip(disks,expected['disks']):close(actual,wanted)
    status='status complete' if exit_code==0 else 'status event_budget_exhausted'
    assert text.splitlines()[-1]==status,text
if __name__=='__main__':
    validate(sys.argv[1],sys.argv[2],int(sys.argv[3]))
    print('validated case '+sys.argv[1])
