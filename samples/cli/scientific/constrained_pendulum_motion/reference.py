#!/usr/bin/env python3
"""Independent 60-digit Decimal oracle; freeze before any Omega execution."""
from decimal import Decimal as D, localcontext
from pathlib import Path
import json, math, argparse
ROOT = Path(__file__).resolve().parent
FIELDS = ['step', 'x', 'y', 'vx', 'vy', 'raw_drift', 'drift', 'tangent', 'energy']
VALID = {'equilibrium': (0,1), 'small_unit': (1,1), 'small_double': (1,2), 'released': (2,1), 'projection': (3,1)}
INVALID = ['zero_length', 'too_long', 'bad_scenario', 'truncated', 'trailing', 'empty']
def calculate(scenario, length):
    with localcontext() as context:
        context.prec = 60
        length, gravity, interval = D(length), D('9.81'), D('.01')
        horizontal = D(0) if scenario == 0 else length * D('.01' if scenario == 1 else '.6' if scenario == 2 else '.66')
        vertical = -length if scenario == 0 else -(length*length-horizontal*horizontal).sqrt() if scenario == 1 else length * D('-.8' if scenario == 2 else '-.88')
        speed_horizontal, speed_vertical = (D('.3'),D('.4')) if scenario == 3 else (D(0),D(0))
        rows = []
        for step in range(33):
            # Derived circle constraint: q' = L q/|q|, v' = v-q'(q'.v)/L².
            raw = horizontal**2 + vertical**2 - length**2
            radial_scale = length / (horizontal**2 + vertical**2).sqrt()
            horizontal, vertical = horizontal*radial_scale, vertical*radial_scale
            component = (horizontal*speed_horizontal+vertical*speed_vertical)/length**2
            speed_horizontal, speed_vertical = speed_horizontal-horizontal*component, speed_vertical-vertical*component
            energy = (speed_horizontal**2+speed_vertical**2)/2+gravity*(vertical+length)
            rows.append([float(v) for v in (step,horizontal,vertical,speed_horizontal,speed_vertical,raw,horizontal**2+vertical**2-length**2,horizontal*speed_horizontal+vertical*speed_vertical,energy)])
            # Twice differentiating q.q=L² gives q.a=-v.v. With a=g_vector-lambda*q,
            # lambda=(v.v-g*y)/L². Kick then drift uses the kicked speed.
            multiplier = (speed_horizontal**2+speed_vertical**2-gravity*vertical)/length**2
            speed_horizontal -= interval*multiplier*horizontal
            speed_vertical -= interval*(gravity+multiplier*vertical)
            horizontal += interval*speed_horizontal
            vertical += interval*speed_vertical
        return rows

def freeze():
    destination = ROOT/'data/oracle.json'
    if destination.exists():
        raise SystemExit('Frozen oracle already exists; refusing overwrite')
    cases = {name: calculate(*parameters) for name,parameters in VALID.items()}
    payload = {'model': 'unit mass, g=9.81, dt=.01, circle kick/drift/project, 32 steps',
               'fields': FIELDS, 'output_absolute_tolerance': 2e-6,
               'small_angle_position_tolerance': 2e-4, 'small_angle_velocity_tolerance': 1e-4,
               'cases': cases, 'invalid': INVALID}
    # Independent linear analytic checks constrain the proposed experiment before execution.
    for name in ['small_unit','small_double']:
        length = VALID[name][1]
        frequency, time = math.sqrt(9.81/length), .32
        final = cases[name][-1]
        assert abs(final[1] - .01*length*math.cos(frequency*time)) < 2e-4
        assert abs(final[3] + .01*length*frequency*math.sin(frequency*time)) < 1e-4
    # A separately derived released-rest first kick: (-.047088,-.035316),
    # unprojected q=(.59952912,-.80035316), then normalize.
    radius = math.hypot(.59952912,-.80035316)
    first = cases['released'][1]
    assert abs(first[1]-.59952912/radius) < 1e-14
    assert abs(first[2]+.80035316/radius) < 1e-14
    assert cases['equilibrium'][-1][1:5] == [0,-1,0,0]
    assert abs(cases['projection'][0][5]-.21) < 1e-14
    assert abs(cases['projection'][0][3]-.384) < 1e-14
    assert abs(cases['projection'][0][4]-.288) < 1e-14
    destination.write_text(json.dumps(payload, indent=2)+'\n')
    print('Frozen 5 trajectories, analytic small-angle and separately derived one-step/projection checks')

def verify(name, output):
    oracle = json.loads((ROOT/'data/oracle.json').read_text())
    text = Path(output).read_text()
    if name in oracle['invalid']:
        assert 'INVALID: expected' in text and 'ROW ' not in text and 'COMPLETE' not in text
        return
    rows = [[int(column)/1e6 for column in line.split()[1:]] for line in text.splitlines() if line.startswith('ROW ')]
    expected = oracle['cases'][name]
    assert len(rows) == 33, len(rows)
    for index,(actual,reference) in enumerate(zip(rows,expected)):
        assert len(actual) == len(FIELDS)
        for field,got,wanted in zip(FIELDS,actual,reference):
            assert math.isfinite(got) and abs(got-wanted) <= oracle['output_absolute_tolerance'], (name,index,field,got,wanted)
    changes = [int(line.split()[1])/1e6 for line in text.splitlines() if line.startswith('ENERGY_CHANGE ')]
    assert len(changes) == 1 and abs(changes[0]-(expected[-1][-1]-expected[0][-1])) <= 2e-6
    assert text.count('COMPLETE 32 steps') == 1
    for row in rows:
        assert abs(row[6]) <= 2e-6 and abs(row[7]) <= 2e-6
    if name.startswith('small_'):
        length, time = VALID[name][1], .32
        frequency = math.sqrt(9.81/length)
        assert abs(rows[-1][1]-.01*length*math.cos(frequency*time)) <= oracle['small_angle_position_tolerance']
        assert abs(rows[-1][3]+.01*length*frequency*math.sin(frequency*time)) <= oracle['small_angle_velocity_tolerance']
    print(name+': 33 rows, constraint, tangent, energy and independent oracle verified')
if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--freeze', action='store_true')
    parser.add_argument('--case', choices=list(VALID)+INVALID)
    parser.add_argument('--output')
    options = parser.parse_args()
    freeze() if options.freeze else verify(options.case, options.output)
