#!/usr/bin/env python3
"""Serial public-entry validation against the pre-execution frozen oracle."""
import argparse, hashlib, json, math, platform, subprocess, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EXPECTED = json.loads((ROOT / 'data/expected.json').read_text())

def verify(name, stdout):
    expected = EXPECTED['cases'][name]
    rows = [line.split() for line in stdout.splitlines() if line.strip()]
    scalars = {}; vertices = {}; certificates = {}; active = set()
    for row in rows:
        if row[0] in ('status', 'vertices', 'area_whole', 'area_fraction_micro'):
            assert row[0] not in scalars, f'duplicate {row}'
            scalars[row[0]] = row[1]
        elif row[0] == 'vertex':
            index = int(row[1]); assert index not in vertices
            vertices[index] = (int(row[2])/1e6, int(row[3])/1e6)
        elif row[0] == 'certificate':
            pair = (int(row[1]), int(row[2])); assert pair not in certificates
            certificates[pair] = int(row[3])/1e4
        elif row[0] == 'active':
            pair = (int(row[1]), int(row[2])); assert pair not in active
            active.add(pair)
        else:
            raise AssertionError(f'unexpected output row {row}')
    policy = EXPECTED['policy']; count = len(expected['vertices'])
    assert int(scalars['vertices']) == count == len(vertices), (name, scalars, vertices)
    assert sorted(vertices) == list(range(count))
    assert scalars['status'] == expected['status'], (name,scalars)
    area = int(scalars['area_whole']) + int(scalars['area_fraction_micro'])/1e6
    assert abs(area-expected['area']) <= policy['area_tolerance'], (name,area,expected['area'])
    actual = [vertices[i] for i in range(count)]
    offset = 0
    if count:
        offsets = [k for k in range(count) if all(math.dist(actual[i], expected['vertices'][(i+k)%count]) <= policy['vertex_tolerance'] for i in range(count))]
        assert offsets, (name, actual, expected['vertices'])
        offset = offsets[0]
        signed = sum(actual[i][0]*actual[(i+1)%count][1]-actual[i][1]*actual[(i+1)%count][0] for i in range(count))/2
        assert signed >= 0 and abs(signed-area) <= 0.00005, (name,signed,area)
    packet = list(map(int,(ROOT/'data'/f'{name}.txt').read_text().split()))
    bound,constraint_count = packet[:2]
    assert len(certificates) == count*constraint_count
    expected_active = set()
    for vertex,(x,y) in enumerate(actual):
        assert abs(x)<=bound+policy['vertex_tolerance'] and abs(y)<=bound+policy['vertex_tolerance']
        for constraint in range(constraint_count):
            a,b,c = packet[2+constraint*3:5+constraint*3]
            residual = a*x+b*y-c/1e6
            # Printed coordinates each lose less than one micro-unit.
            assert residual <= (abs(a)+abs(b))*1.00001e-6 + policy['inside_epsilon']*2, (name,vertex,constraint,residual)
            assert abs(certificates[vertex,constraint]-residual) <= policy['certificate_tolerance']+(abs(a)+abs(b))*1e-6
        for constraint in expected['active'][(vertex+offset)%count]: expected_active.add((vertex,constraint))
    assert active == expected_active, (name,active,expected_active)
    return {'vertices':count,'area':area,'certificates':len(certificates),'active':len(active)}

def main():
    parser=argparse.ArgumentParser(); parser.add_argument('--omega',required=True); parser.add_argument('--mode',choices=['native','both'],default='both'); parser.add_argument('--only'); parser.add_argument('--executable',help='Previously compiled native program; native mode only'); options=parser.parse_args()
    assert not options.executable or options.mode=='native'
    evidence=ROOT/'build'/'validation'/(options.mode+('-'+options.only if options.only else ''));  evidence.mkdir(parents=True,exist_ok=True)
    results=[]
    def invoke(label,command,stdin=b''):
        start=time.monotonic(); run=subprocess.run(command,input=stdin,capture_output=True,timeout=240)
        (evidence/f'{label}.stdout').write_bytes(run.stdout); (evidence/f'{label}.stderr').write_bytes(run.stderr)
        row={'label':label,'command':command,'exit':run.returncode,'seconds':time.monotonic()-start,'stdout_sha256':hashlib.sha256(run.stdout).hexdigest()};results.append(row)
        return run,row
    try:
        run,_=invoke('check',[options.omega,'--check',str(ROOT/'main.omg')]);assert run.returncode==0,run.stderr.decode()
        names=[options.only] if options.only else list(EXPECTED['cases'])
        for name in names:
            stdin=(ROOT/'data'/f'{name}.txt').read_bytes();assert hashlib.sha256(stdin).hexdigest()==EXPECTED['cases'][name]['input_sha256']
            command=([str(Path(options.executable).resolve())] if options.executable else [options.omega,'run','--keep']+(['--both'] if options.mode=='both' else [])+[str(ROOT/'main.omg')])
            run,row=invoke(name,command,stdin);assert run.returncode==0,run.stderr.decode()
            row['checks']=verify(name,run.stdout.decode())
            if options.mode=='both':
                text=run.stderr.decode()+run.stdout.decode()
                assert 'interp exit: 0' in text, text
        if not options.only:
            for name in EXPECTED['invalid']:
                command=([str(Path(options.executable).resolve())] if options.executable else [options.omega,'run','--keep']+(['--both'] if options.mode=='both' else [])+[str(ROOT/'main.omg')])
                run,row=invoke('invalid_'+name,command,(ROOT/'data'/f'invalid_{name}.txt').read_bytes())
                assert run.returncode==2 and run.stdout.decode()=='invalid input\n', (name,run.returncode,run.stdout,run.stderr)
                if options.mode=='both': assert 'interp exit: 2' in run.stderr.decode(), run.stderr.decode()
            run,row=invoke('source_invalid',[options.omega,'--check',str(ROOT/'intended_invalid/main.omg')])
            assert run.returncode!=0 and (ROOT/'intended_invalid/expected.txt').read_text().strip() in (run.stdout+run.stderr).decode(), (run.returncode,run.stdout,run.stderr)
        print(json.dumps({'validated':len(results),'mode':options.mode,'results':results},indent=2))
    finally:
        record={'host':platform.platform(),'executable_sha256':hashlib.sha256(Path(options.executable).read_bytes()).hexdigest() if options.executable else None,'oracle_sha256':hashlib.sha256((ROOT/'data/expected.json').read_bytes()).hexdigest(),'results':results}
        (evidence/'results.json').write_text(json.dumps(record,indent=2)+'\n')

if __name__=='__main__':main()
