#!/usr/bin/env python3
"""Independent exact packet tree. Group arrivals before scattering; rescan all
pending packet groups for stored energy rather than using Omega's running ledger.
Only --freeze writes the expected fixture. No Omega execution occurs here.
"""
from fractions import Fraction as F
from collections import defaultdict
from pathlib import Path
import hashlib, json, sys
ROOT = Path(__file__).resolve().parent
PRESETS = [
    (50, 50, F(0), F(0), 1, 1, False),
    (50, 100, F(1,2), F(0), 1, 2, False),
    (50, 100, F(1,2), F(1), 1, 1, False),
    (50, 100, F(1,2), F(-1), 1, 1, False),
    (100, 50, F(-1,2), F(1,2), 2, 3, False),
    (50, 100, F(1,2), F(1), 1, 1, True),
]
def solve(identifier):
    z1,z2,rs,rl,d1,d2,alternate = PRESETS[identifier]
    reflection = F(z2-z1,z2+z1)
    # Each entry retains individual tree branches until a physical arrival.
    pending = defaultdict(list)
    supplied = source_heat = load_heat = F(0)
    records=[]
    simultaneous=[]
    for tick in range(24):
        packets = [pending.pop((tick,lane),[]) for lane in range(4)]
        a,b,s,l = [sum(values,F(0)) for values in packets]
        c = reflection*a+(1-reflection)*b
        d = (1+reflection)*a-reflection*b
        drive = F(1) if tick < 2 else F(-1) if alternate and tick in (4,5) else F(0)
        so,lo = rs*s+drive,rl*l
        if a and b: simultaneous.append(tick)
        assert a+c == b+d
        assert (a-c)/z1 == (d-b)/z2
        assert a*a/z1+b*b/z2 == c*c/z1+d*d/z2
        # Follow each branch independently, preserving its signed amplitude.
        for value in packets[0]:
            pending[(tick+d1,2)].append(reflection*value)
            pending[(tick+d2,3)].append((1+reflection)*value)
        for value in packets[1]:
            pending[(tick+d1,2)].append((1-reflection)*value)
            pending[(tick+d2,3)].append(-reflection*value)
        for value in packets[2]: pending[(tick+d1,0)].append(rs*value)
        for value in packets[3]: pending[(tick+d2,1)].append(rl*value)
        if drive: pending[(tick+d1,0)].append(drive)
        supplied += (so*so-(rs*s)**2)/z1
        source_heat += (s*s-(rs*s)**2)/z1
        load_heat += (l*l-lo*lo)/z2
        travelling = sum((sum(values,F(0))**2/(z1 if lane in (0,2) else z2)
                          for (_,lane),values in pending.items()),F(0))
        residual=supplied-source_heat-load_heat-travelling
        assert residual == 0
        records.extend([['W',tick,a,b,c,d,s,so,l,lo],['V',tick,s+so,a+c,l+lo],
                        ['E',tick,supplied,source_heat,load_heat,travelling,residual]])
    return records,simultaneous

def freeze():
    result={'model':'lossless two-section sampled travelling waves v1',
            'horizon':24,'scale':1000000,'absolute_tolerance': '1/500000',
            'residual_tolerance':'1/1000000','cases':{}}
    for identifier in range(6):
        rows,simultaneous=solve(identifier)
        result['cases'][str(identifier)]={'rows':[[row[0],row[1],*[str(x) for x in row[2:]]] for row in rows],
                                        'simultaneous_ticks':simultaneous}
        (ROOT/'data'/f'case_{identifier}.stdin').write_text(f'{identifier} 24\n')
        csv='\n'.join(','.join([row[0],str(row[1]),*[str(int(x*1000000)) for x in row[2:]]]) for row in rows)+'\n'
        (ROOT/'data'/f'case_{identifier}.expected.csv').write_text(csv)
    oracle=ROOT/'data/oracle.json'
    oracle.write_text(json.dumps(result,indent=2)+'\n')
    frozen={'oracle_sha256':hashlib.sha256(oracle.read_bytes()).hexdigest(),
            'reference_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'policy':'Frozen before first Omega check/run; all expected values exact rational; tolerances immutable during validation.'}
    (ROOT/'data/freeze.json').write_text(json.dumps(frozen,indent=2)+'\n')
    print(json.dumps(frozen))
if __name__=='__main__':
    if sys.argv[1:]!=['--freeze']: raise SystemExit('use --freeze explicitly before Omega execution')
    freeze()
