"""Exact rational conservation coefficients for every non-identifiable split.
Regenerate only with --freeze, before executing newly authored closure outputs.
The original high-precision root oracle is read, never modified here.
"""
from fractions import Fraction
from pathlib import Path
import hashlib,json,sys
project=Path(__file__).resolve().parent

def coefficients():
    raw=(project/'data/oracle.json').read_bytes()
    assert hashlib.sha256(raw).hexdigest()==(project/'data/oracle.sha256').read_text().split()[0]
    cases={}
    for case in json.loads(raw)['cases']:
        if case['expected']['status']!='non_identifiable':continue
        tokens=case['input'].split();feed=list(map(Fraction,tokens[1::2]));ratios=list(map(Fraction,tokens[2::2]))
        assert all(ratio==1 for abundance,ratio in zip(feed,ratios) if abundance)
        liquid=feed;vapor=[ratio*abundance for ratio,abundance in zip(ratios,feed)]
        constant=[composition-abundance for composition,abundance in zip(liquid,feed)]
        linear=[gas-fluid for gas,fluid in zip(vapor,liquid)]
        assert all(value==0 for value in constant+linear)
        cases[case['name']]={'constant_coefficients':list(map(str,constant)),'beta_coefficients':list(map(str,linear))}
    return {'identity':'component closure is zero for every admissible split; phase presence remains undetermined','cases':cases}

if __name__=='__main__':
    result=coefficients()
    if sys.argv[1:]==['--freeze']:
        raw=(json.dumps(result,indent=2)+'\n').encode()
        (project/'data/closure_oracle.json').write_bytes(raw)
        (project/'data/closure_oracle.sha256').write_text(hashlib.sha256(raw).hexdigest()+'  closure_oracle.json\n')
    else:
        assert sys.argv[1:]==[]
        assert result==json.loads((project/'data/closure_oracle.json').read_bytes())
        print('2 frozen exact rational closure identities verified')
