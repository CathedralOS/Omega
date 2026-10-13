"""Independent Fermat minimization, frozen before Omega execution.

Horizontal distance a in each down/up leg minimizes
2 sum sqrt(h*h+a*a)/v + (x-2 sum a)/refractor_speed.
Bisection finds the zero derivative of each strictly convex leg functional;
no Snell cosine/intercept formula from the application is used here.
Two-layer crossover is separately analytic, not fitted from reported times.
"""
from decimal import Decimal as D, localcontext
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent


def fermat(velocities, thicknesses, refractor):
    speed = velocities[refractor]
    horizontal, durations = [], []
    for velocity, height in zip(velocities[:refractor], thicknesses):
        lower, upper = D(0), 2 * height * speed / (speed - velocity)
        for _ in range(240):
            middle = (lower + upper) / 2
            derivative = middle / (velocity * (height*height + middle*middle).sqrt()) - 1 / speed
            if derivative < 0:
                lower = middle
            else:
                upper = middle
        distance = (lower + upper) / 2
        horizontal.append(distance)
        durations.append((height*height + distance*distance).sqrt()/velocity)
    critical = 2 * sum(horizontal)
    legs = 2 * sum(durations)
    return critical, legs


def solve(velocities, thicknesses, offsets):
    velocities = list(map(D, velocities)); thicknesses = list(map(D, thicknesses)); offsets = list(map(D, offsets))
    branches = [fermat(velocities, thicknesses, j) for j in range(1, len(velocities))]
    rows = []
    for j, (critical, legs) in enumerate(branches, 1):
        rows.append(['B', j, str(critical), str(legs-critical/velocities[j])])
    for index, distance in enumerate(offsets):
        best = distance/velocities[0]; winner = 0
        rows.append(['R', index, 0, str(distance), str(best)])
        for j, (critical, legs) in enumerate(branches, 1):
            # Decimal minimization may be a sub-1e-60 amount off an exact boundary.
            if distance + D('1e-55') >= critical:
                time = legs + (distance-critical)/velocities[j]
                rows.append(['R', index, j, str(distance), str(time)])
                if time < best-D('1e-55')*best:
                    best, winner = time, j
        rows.append(['F', index, winner, str(distance), str(best)])
    return rows


def freeze():
    valid = {
        'single': (['3'], [], ['0','1','123.456']),
        'activation_crossover': (['3','5'], ['2'], ['0','2.999999','3','3.000001','7.999999','8','8.000001','20']),
        'unit_scale': (['3000','5000'], ['2000'], ['0','2999.999','3000','3000.001','7999.999','8000','8000.001','20000']),
        'length_scale': (['3','5'], ['20'], ['0','30','80','200']),
        'decimal': (['1.25','2.75'], ['0.125'], ['0','0.1','0.5','1','10']),
        'multiple': (['2','3','5','8'], ['1','2','3'], list(map(str,[0,1,2,3,4,5,8,10,12,15,20,30,40,60,80,100]))),
        'close_velocity': (['100','100.000001'], ['0.01'], ['0','100','200','300','1000']),
        'no_offsets': (['3','5'], ['2'], []),
        'tiny_time': (['500000','1000000'], ['0.000001'], ['0','0.000003','0.000004','0.00001']),
        'extreme_gap': (['999999.999999','1000000'], ['0.1'], ['0','100000','141421.356','141422','300000']),
    }
    invalid = {
        'empty': ('',64), 'nonfinite_nan': ('2 1 3 NaN 2 8\n',64),
        'nonfinite_inf': ('2 1 3 Inf 2 8\n',64), 'overflow': ('1 1 9999999999999 0\n',64),
        'negative': ('1 1 -3 0\n',64), 'negative_offset': ('1 1 3 -1\n',64),
        'zero_velocity': ('1 1 0 0\n',65), 'descending': ('2 1 5 3 2 8\n',65),
        'equal_velocity': ('2 1 3 3 2 8\n',65), 'zero_thickness': ('2 1 3 5 0 8\n',65),
        'negative_thickness': ('2 1 3 5 -2 8\n',64), 'layers_excess': ('5 0\n',65),
        'offsets_excess': ('1 17\n',65), 'fractional_count': ('1.5 0 3 5\n',64),
        'missing': ('2 1 3 5 2\n',64), 'surplus': ('1 1 3 0 9\n',64),
        'long_input': (' '*1024,64), 'bad_decimal': ('1 1 3..0 0\n',64),
        'offset_range': ('1 1 3 1000000.1\n',65),
    }
    records = {}
    for name, (velocities, thicknesses, offsets) in valid.items():
        text = ' '.join([str(len(velocities)),str(len(offsets)),*velocities,*thicknesses,*offsets])+'\n'
        (ROOT/'data'/f'{name}.stdin').write_text(text)
        records[name] = dict(exit=0, rows=solve(velocities, thicknesses, offsets))
    # Same physical acquisition with EOF and CRLF whitespace variants.
    for name, text in [('eof','2 1 3 5 2 8'),('crlf','2\t1\r\n3 5\r\n2\t8\r\n')]:
        (ROOT/'data'/f'{name}.stdin').write_bytes(text.encode())
        records[name]=dict(exit=0,rows=solve(['3','5'],['2'],['8']))
    for name, (text, code) in invalid.items():
        (ROOT/'data'/f'{name}.stdin').write_text(text)
        records[name]=dict(exit=code, error='ERROR syntax' if code==64 else 'ERROR range')
    for name, record in records.items():
        record['input_sha256']=hashlib.sha256((ROOT/'data'/f'{name}.stdin').read_bytes()).hexdigest()
    # Closed form direct/head crossover, independent of Fermat's result.
    crossovers=[]
    for velocities, heights, offsets in valid.values():
        if len(velocities)==2:
            first,second=map(D,velocities); height=D(heights[0])
            crossover=2*height*((second+first)/(second-first)).sqrt()
            probes=[crossover*(1-D('1e-10')),crossover,crossover*(1+D('1e-10'))]
            winners=[row[2] for row in solve(velocities,heights,probes) if row[0]=='F']
            assert winners==[0,0,1], (velocities,crossover,winners)
            crossovers.append(dict(velocities=velocities,thickness=heights[0],offset=str(crossover),probe_offsets=list(map(str,probes)),expected_winners=winners))
    payload=dict(method='strictly convex Fermat leg minimization; Decimal 70 digits, 240 bisections',absolute_tolerance='0',relative_tolerance='1e-7',crossovers=crossovers,cases=records)
    (ROOT/'validation/oracle.json').write_text(json.dumps(payload,indent=2)+'\n')
    print(hashlib.sha256((ROOT/'validation/oracle.json').read_bytes()).hexdigest())

if __name__=='__main__':
    with localcontext() as context:
        context.prec=70
        freeze()
