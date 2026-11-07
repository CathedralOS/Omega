"""Freeze analytic periodic binomial convolutions; never consumes application output."""
import hashlib, json, math
from pathlib import Path
from decimal import Decimal, localcontext
ROOT = Path(__file__).resolve().parent

def oracle(field, velocity, spacing, timestep, steps):
    with localcontext() as context:
        context.prec = 80
        initial = [Decimal(str(value)) for value in field]
        fraction = abs(Decimal(str(velocity))) * Decimal(str(timestep)) / Decimal(str(spacing))
        direction = 1 if velocity >= 0 else -1
        weights = [Decimal(math.comb(steps, offset)) * fraction ** offset * (1-fraction) ** (steps-offset)
                   for offset in range(steps+1)] if 0 < fraction < 1 else None
        if fraction == 0: final = initial
        elif fraction == 1: final = [initial[(cell-direction*steps)%len(initial)] for cell in range(len(initial))]
        else: final = [sum(weight * initial[(cell-direction*offset)%len(initial)] for offset, weight in enumerate(weights)) for cell in range(len(initial))]
        return dict(field=list(map(str,final)),mass=str(sum(initial)*Decimal(str(spacing))),minimum=str(min(final)),maximum=str(max(final)),cfl=str(Decimal(str(velocity))*Decimal(str(timestep))/Decimal(str(spacing))))

def freeze():
    cases=[]
    def add(name, field, velocity, spacing, timestep, steps):
        values=[len(field),steps,velocity,spacing,timestep,*field]
        stdin=' '.join(map(str,values))+'\n'
        cases.append(dict(name=name,stdin=stdin,initial=field,spacing=spacing,exit=0,expected=oracle(field,velocity,spacing,timestep,steps)))
    for sign in (1,-1):
        add(f'constant_{sign:+d}'.replace('+','p').replace('-','m'),[2.25]*4,sign*0.5,0.25,0.25,256)
        add(f'impulse_{sign:+d}'.replace('+','p').replace('-','m'),[1.0]+[0.0]*7,sign*0.75,1,1,17)
        add(f'sinusoid_{sign:+d}'.replace('+','p').replace('-','m'),[round(1+0.8*math.sin(2*math.pi*i/64),14) for i in range(64)],sign*0.4,0.5,0.5,256)
        add(f'shift_{sign:+d}'.replace('+','p').replace('-','m'),[0,1,2,4,8,16,32],sign*2,0.5,0.25,11)
    add('zero_velocity',[0,0.125,3,4],0,0.3,2,256)
    add('zero_steps',[0,1,2,3],-0.7,1,1,0)
    add('minimum_impulse',[1,0,0,0],0.25,1,1,1)
    add('maximum_impulse',[1]+[0]*63,-0.999,1,1,256)
    add('equal_tiny_geometry',[1,0,0,0],1,1e-309,1e-309,1)
    add('balanced_cfl_scales',[1,0,0,0],1e-100,1e100,1e200,1)
    add('subnormal_impulse',[5e-324,0,0,0],0.5,1,1,1)
    add('wide_mass',[1e308]*4,0,1,1,0)
    add('tiny_mass',[1e-200]*4,0,1e-200,1e-200,0)
    add('scaled_large',[1e100,2e100,0,3e100],0.5,1,1,15)
    add('scaled_small',[1e-100,2e-100,0,3e-100],-0.5,1,1,15)
    invalid={
      'unstable_positive':'4 1 1.01 1 1 1 0 0 0', 'unstable_negative':'4 1 -1.01 1 1 1 0 0 0',
      'cells_low':'3 1 0.5 1 1 1 0 0', 'cells_high':'65 1 0.5 1 1', 'fractional_cells':'4.5 1 0.5 1 1 0 0 0 0',
      'steps_high':'4 257 0.5 1 1 1 0 0 0', 'negative_steps':'4 -1 0.5 1 1 1 0 0 0',
      'spacing_zero':'4 1 0.5 0 1 1 0 0 0', 'spacing_negative':'4 1 0.5 -1 1 1 0 0 0',
      'timestep_zero':'4 1 0.5 1 0 1 0 0 0', 'timestep_negative':'4 1 0.5 1 -1 1 0 0 0',
      'negative_underflow':'4 1 0.5 1 1 -1e-400 0 0 0', 'negative_field':'4 1 0.5 1 1 1 -0.1 0 0', 'nan':'4 1 nan 1 1 1 0 0 0',
      'infinite':'4 1 0.5 1 1 1e400 0 0 0', 'missing':'4 1 0.5 1 1 1 0 0',
      'extra':'4 1 0.5 1 1 1 0 0 0 0', 'empty':'', 'malformed':'4 1 0.5 1 1 1..2 0 0 0',
      'overlong':' ' * 4097}
    for name,stdin in invalid.items(): cases.append(dict(name=name,stdin=stdin,exit=64 if name in ('nan','empty','malformed','overlong') else 65))
    data=ROOT/'data'; data.mkdir(exist_ok=True)
    for case in cases: (data/(case['name']+'.stdin')).write_text(case['stdin'])
    payload=json.dumps(cases,indent=2,sort_keys=True)+'\n'
    (data/'oracle.json').write_text(payload)
    paths=[ROOT/'reference.py',data/'oracle.json',*sorted(data.glob('*.stdin'))]
    (data/'freeze.sha256').write_text(''.join(hashlib.sha256(path.read_bytes()).hexdigest()+'  '+str(path.relative_to(ROOT))+'\n' for path in paths))
if __name__=='__main__': freeze()
