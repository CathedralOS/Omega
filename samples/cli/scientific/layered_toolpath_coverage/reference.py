#!/usr/bin/env python3
"""Independent exact set/interval oracle; freeze mode never invokes Omega."""
import argparse, hashlib, json, subprocess
from collections import Counter
from pathlib import Path
ROOT = Path(__file__).resolve().parent

def rectangle(x0,y0,x1,y1,z):
    return {(x,y,z) for x in range(x0,x1) for y in range(y0,y1)}

def cases():
    solid=rectangle(0,0,4,3,0)
    hole=rectangle(0,0,6,4,0)-rectangle(2,1,4,3,0)
    islands=rectangle(0,0,2,2,0)|rectangle(5,1,7,3,0)
    layers=rectangle(0,0,5,4,0)| (rectangle(0,0,5,4,1)-rectangle(1,1,4,3,1)) | rectangle(2,1,3,3,2)
    return {
        'rectangle':(4,3,1,0,1,solid),
        'hole':(6,4,1,0,1,hole),
        'islands':(7,3,1,0,1,islands),
        'layers':(5,4,3,0,1,layers),
        'vertical_hole':(6,4,1,1,1,hole),
        'sparse':(4,3,1,0,2,solid),
        'empty':(3,2,2,1,2,set()),
        'unit':(1,1,1,1,1,{(0,0,0)}),
        'maximum':(8,8,4,0,1,set().union(*(rectangle(0,0,8,8,z) for z in range(4)))),
    }

def oracle(case):
    width,height,layers,orientation,stride,region=case
    # Group coordinates into mathematical intervals rather than streaming a mask.
    groups={}
    for x,y,z in region:
        lane,along=(y,x) if orientation==0 else (x,y)
        if lane%stride==0: groups.setdefault((z,lane),set()).add(along)
    records=[]; previous=(0,0,0)
    for (z,lane),positions in sorted(groups.items()):
        starts=sorted(p for p in positions if p-1 not in positions)
        ends=sorted(p+1 for p in positions if p+1 not in positions)
        for start,end in zip(starts,ends):
            a=(start,lane,z) if orientation==0 else (lane,start,z)
            b=(end,lane,z) if orientation==0 else (lane,end,z)
            if a!=previous: records.append(('T',previous,a))
            records.append(('D',a,b)); previous=b
    visits=Counter(); travel=0
    for kind,a,b in records:
        if kind=='T': travel+=sum(abs(x-y) for x,y in zip(a,b)); continue
        x,y,z=a; ex,ey,ez=b
        cells={(i,y,z) for i in range(x,ex)} if orientation==0 else {(x,i,z) for i in range(y,ey)}
        visits.update(cells)
    audit=dict(region=len(region),covered=len(region&visits.keys()),missed=len(region-visits.keys()),
               waste=sum(v for p,v in visits.items() if p not in region),duplicate=sum(max(v-1,0) for v in visits.values()),
               deposition=sum(visits.values()),travel=travel,D=sum(r[0]=='D' for r in records),T=sum(r[0]=='T' for r in records))
    text=''.join(k+' '+' '.join(f'{n:04d}' for n in a+b)+'\n' for k,a,b in records)
    text+='AUDIT '+' '.join(f'{k}={v:04d}' for k,v in audit.items())+'\n'
    return text,audit

def freeze():
    manifest={'model':'unit cells; axis aligned maximal runs; Manhattan travel; exact integers','tolerance':0,'cases':{}}
    for name,case in cases().items():
        w,h,l,o,s,region=case
        data=f'{w}{h}{l}{o}{s}\n'+''.join(''.join('1' if (x,y,z) in region else '0' for x in range(w))+'\n' for z in range(l) for y in range(h))
        expected,audit=oracle(case)
        (ROOT/'data'/f'{name}.in').write_text(data)
        (ROOT/'data'/f'{name}.out').write_text(expected)
        manifest['cases'][name]={'input_sha256':hashlib.sha256(data.encode()).hexdigest(),'output_sha256':hashlib.sha256(expected.encode()).hexdigest(),'audit':audit}
    invalid={'zero_width':'03101\n','bad_orientation':'43121\n','bad_stride':'43100\n','too_many_layers':'43501\n',
             'bad_cell':'11101\n2\n','truncated':'22101\n111\n','extra':'11101\n1x\n','extra_digit':'11101\n11\n','nondigit':'x1101\n1\n','eof':''}
    for name,data in invalid.items(): (ROOT/'data'/f'invalid_{name}.in').write_text(data)
    (ROOT/'ORACLE.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print('FROZEN',len(manifest['cases']),'valid;',len(invalid),'invalid; tolerance=0')

def inspect_output(text,case):
    w,h,l,o,s,region=case; previous=(0,0,0); visits=Counter(); lengths={'D':0,'T':0}; counts={'D':0,'T':0}
    lines=text.splitlines()
    for line in lines[:-1]:
        words=line.split(); assert len(words)==7 and words[0] in lengths,line
        kind=words[0]; a=tuple(map(int,words[1:4])); b=tuple(map(int,words[4:7]))
        assert a==previous,('discontinuous',a,previous)
        assert all(0<=x<=w and 0<=y<=h and 0<=z<l for x,y,z in (a,b))
        counts[kind]+=1; lengths[kind]+=sum(abs(x-y) for x,y in zip(a,b)); previous=b
        if kind=='D':
            assert a[2]==b[2] and ((o==0 and a[1]==b[1] and a[0]<b[0]) or (o==1 and a[0]==b[0] and a[1]<b[1]))
            assert (a[1] if o==0 else a[0])%s==0
            if o==0: visits.update((x,a[1],a[2]) for x in range(a[0],b[0]))
            else: visits.update((a[0],y,a[2]) for y in range(a[1],b[1]))
    observed={k:int(v) for k,v in (pair.split('=') for pair in lines[-1].split()[1:])}
    assert lines[-1].startswith('AUDIT ')
    independent={'region':len(region),'covered':len(region&visits.keys()),'missed':len(region-visits.keys()),
      'waste':sum(v for p,v in visits.items() if p not in region),'duplicate':sum(max(v-1,0) for v in visits.values()),
      'deposition':lengths['D'],'travel':lengths['T'],**counts}
    assert observed==independent,(observed,independent)
    assert not (visits.keys()-region) and all(v==1 for v in visits.values())

def validate(binary,both,executable=False):
    manifest_bytes=(ROOT/'ORACLE.json').read_bytes()
    assert hashlib.sha256(manifest_bytes).hexdigest()=='d4f566be2f960fa708f16d9cd039950c65c4c890969d258e3245ddd8d1950a6e', 'frozen manifest changed'
    manifest=json.loads(manifest_bytes)
    for name,row in manifest['cases'].items():
        for extension,key in [('in','input_sha256'),('out','output_sha256')]:
            assert hashlib.sha256((ROOT/'data'/f'{name}.{extension}').read_bytes()).hexdigest()==row[key], (name,'frozen fixture changed')
    folder=ROOT/'build'/('both' if both else 'native'); folder.mkdir(parents=True,exist_ok=True)
    results=[]
    for name,case in cases().items():
        input_data=(ROOT/'data'/f'{name}.in').read_bytes(); expected=(ROOT/'data'/f'{name}.out').read_bytes()
        command=[binary] if executable else [binary,'run']+(['--both'] if both else [])+[str(ROOT/'main.omg')]
        run=subprocess.run(command,input=input_data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=600)
        (folder/f'{name}.stdout').write_bytes(run.stdout); (folder/f'{name}.stderr').write_bytes(run.stderr)
        assert run.returncode==0,(name,run.returncode,run.stderr.decode())
        if executable: assert run.stderr==b'', (name,run.stderr)
        assert run.stdout==expected,(name,'output mismatch',run.stdout.decode(),expected.decode())
        inspect_output(run.stdout.decode(),case)
        if both: assert b'interp exit: 0' in run.stderr and b'DECLINED' not in run.stderr,(name,run.stderr)
        results.append({'case':name,'command':command,'exit':run.returncode,'output_sha256':hashlib.sha256(run.stdout).hexdigest()})
        print('PASS',name,flush=True)
    for path in sorted((ROOT/'data').glob('invalid_*.in')):
        command=[binary] if executable else [binary,'run']+(['--both'] if both else [])+[str(ROOT/'main.omg')]
        run=subprocess.run(command,input=path.read_bytes(),stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=600)
        (folder/f'{path.stem}.stdout').write_bytes(run.stdout); (folder/f'{path.stem}.stderr').write_bytes(run.stderr)
        assert run.returncode==64 and run.stdout==b'ERROR input\n',(path.name,run.returncode,run.stdout,run.stderr)
        if both: assert b'interp exit: 64' in run.stderr and b'DECLINED' not in run.stderr
        results.append({'case':path.stem,'command':command,'exit':run.returncode})
        print('PASS',path.stem,flush=True)
    (folder/'results.json').write_text(json.dumps(results,indent=2)+'\n')

if __name__=='__main__':
    parser=argparse.ArgumentParser(); parser.add_argument('--freeze',action='store_true'); parser.add_argument('--cli'); parser.add_argument('--both',action='store_true'); parser.add_argument('--executable',action='store_true'); args=parser.parse_args()
    if args.freeze: freeze()
    elif args.cli: validate(args.cli,args.both,args.executable)
    else: parser.error('choose --freeze or --cli')
