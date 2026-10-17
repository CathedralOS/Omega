"""Freeze decimal reporting probes with the original independent convolution."""
import hashlib, json
from decimal import Decimal
from pathlib import Path
from reference import oracle
ROOT=Path(__file__).resolve().parent

def freeze():
    cases=[]
    for name,velocity,field in [
        ('constant18_positive',Decimal('.3'),['.123456789012345678']*4),
        ('constant18_negative',Decimal('-.3'),['.123456789012345678']*4),
        ('narrow18_negative',Decimal('-.3'),['.123456789012345678','.123456789012345679','.123456789012345680','.123456789012345681']),
    ]:
        stdin=' '.join(['4','256',str(velocity),'1','1',*field])+'\n'
        cases.append(dict(name=name,stdin=stdin,initial=field,spacing=1,exit=0,expected=oracle(field,velocity,1,1,256)))
    data=ROOT/'data'
    for case in cases: (data/(case['name']+'.stdin')).write_text(case['stdin'])
    (data/'supplemental_oracle.json').write_text(json.dumps(cases,indent=2,sort_keys=True)+'\n')
    paths=[ROOT/'supplemental_reference.py',data/'supplemental_oracle.json',*(data/(case['name']+'.stdin') for case in cases)]
    (data/'supplemental_freeze.sha256').write_text(''.join(hashlib.sha256(path.read_bytes()).hexdigest()+'  '+str(path.relative_to(ROOT))+'\n' for path in paths))
if __name__=='__main__': freeze()
