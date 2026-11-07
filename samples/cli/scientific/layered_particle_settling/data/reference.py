#!/usr/bin/env python3
"""Independent exact-rational discrete oracle; analytic terminal speeds.
Run with --freeze once, before any Omega execution. Validation reads frozen.json
and never calls this calculator. This file is not the application entrypoint.
"""
from fractions import Fraction as Q
from pathlib import Path
import hashlib
import json
import math
import sys

ROOT = Path(__file__).resolve().parent
CASES = [
    ('homogeneous', 1, 4), ('settle', 2, 4), ('neutral_rest', 2, 2),
    ('rise', 3, 1), ('neutral_relaxation', 4, 2), ('refined', 5, 4),
    ('bottom_bound', 6, 8), ('middle_neutral', 2, 3), ('dense', 2, 8),
]
LAYERS = [(2, 2, False), (3, 1, True), (4, 4, False)]
def region(mode, depth):
    if mode in (1, 4):
        return 0
    return 0 if depth < Q(1,2) else 1 if depth < 2 else 2

def signed(value):
    return f'{value:+09d}'
def micro(value):
    return int(value * 1000000)
def net(density, fluid, velocity):
    rho, coefficient, quadratic = fluid
    return density - rho - coefficient * velocity * (abs(velocity) if quadratic else 1)
def regime(index, density):
    rows = [f'REGIME {signed(index)} {"quadratic" if index == 1 else "linear"}']
    if density == LAYERS[index][0]:
        rows.append(f'NEUTRAL {signed(index)}')
    return rows

def calculate(mode, density):
    depth, velocity, dt, steps = Q(0), Q(0), Q(1,2), 8
    if mode == 3:
        depth, dt, steps = Q(3), Q(1,4), 16
    if mode == 4:
        velocity = Q(1)
    if mode == 5:
        dt, steps = Q(1,4), 16
    if mode == 6:
        depth, velocity = Q(31,4), Q(2)
    rows = ['MODEL unit-volume unit-gravity semi-implicit sampled-layers microunits', f'INPUT {signed(mode)} {signed(density)}']
    balances = []
    for index, fluid in enumerate(LAYERS):
        rho, coefficient, quadratic = fluid
        drive = density-rho
        speed = math.copysign(math.sqrt(abs(drive)/coefficient), drive) if quadratic else drive/coefficient
        balances.append({'layer':index,'speed':speed,'force_residual':net(density,fluid,speed)})
        rows.append(f'BALANCE {signed(index)} {signed(micro(speed))} {signed(0)}')
    index = region(mode, depth)
    rows.extend(regime(index,density))
    traces = []
    def trace(step,index,force,residual):
        traces.append({'step':step,'layer':index,'depth':str(depth),'velocity':str(velocity),'force':str(force),'residual':str(residual)})
        values=[step,index,micro(depth),micro(velocity),micro(force),micro(residual)]
        rows.append('TRACE '+' '.join(map(signed,values)))
    trace(0,index,Q(0),Q(0))
    for step in range(1,steps+1):
        before=region(mode,depth)
        old=velocity
        force=net(density,LAYERS[before],old)
        velocity=old+dt*force/density
        depth+=dt*velocity
        residual=density*(velocity-old)/dt-force
        assert residual==0
        bound='bottom' if depth>=8 else 'top' if depth<=-8 else None
        if bound:
            depth=Q(8 if bound=='bottom' else -8)
            trace(step,before,force,residual)
            rows.append('BOUND '+bound)
            break
        after=region(mode,depth)
        if before!=after:
            rows.append(f'CROSS {signed(step)} {signed(before)} {signed(after)}')
            rows.extend(regime(after,density))
        trace(step,before,force,residual)
    rows.append('DONE')
    return {'mode':mode,'density':density,'balances':balances,'traces':traces,'stdout':'\n'.join(rows)+'\n','exit':0}

if __name__=='__main__':
    if sys.argv[1:] != ['--freeze']:
        raise SystemExit('usage: reference.py --freeze (refuses to overwrite frozen oracle)')
    destination=ROOT/'frozen.json'
    if destination.exists():
        raise SystemExit('already frozen; do not rewrite expected results after execution')
    document={'model':'unit volume/gravity; exact rational semi-implicit steps; sampled start layer',
              'tolerances':{'integer_microunits':1,'force_residual_absolute':1e-8,'terminal_speed_absolute':1e-8},
              'cases':{name:calculate(mode,density) for name,mode,density in CASES}}
    destination.write_text(json.dumps(document,indent=2)+'\n')
    for name,mode,density in CASES:
        (ROOT/(name+'.stdin')).write_text(f'{mode} 0{density}\n')
    invalid={'zero_density':'2 00\n','large_density':'2 09\n','negative_density':'2 -1\n','mode':'7 04\n','trailing':'2 04x\n','empty':'','short':'2\n','full':'2 04'+'x'*20+'\n','non_ascii':'2 0é\n'}
    document_invalid={'stdout':'INVALID expected M 0D with M=1..6 density=1..8\n','exit':64,'inputs':invalid}
    (ROOT/'invalid.json').write_text(json.dumps(document_invalid,indent=2)+'\n')
    (ROOT/'frozen.sha256').write_text(hashlib.sha256(destination.read_bytes()).hexdigest()+'  frozen.json\n')
    for name,case in document['cases'].items():
        print(name,'final',case['traces'][-1], 'crossings',case['stdout'].count('CROSS'))
