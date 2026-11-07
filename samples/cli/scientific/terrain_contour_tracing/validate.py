#!/usr/bin/env python3
"""Compare real executable output against the pre-execution frozen oracle.
No Omega compile, policy acceptance or oracle regeneration occurs here.
"""
import argparse, hashlib, json, re, subprocess
from pathlib import Path
from data.reference import canonical
ROOT=Path(__file__).resolve().parent

def observe(stdout):
    lines=stdout.decode('ascii').splitlines()
    match=re.fullmatch(r'TOPO segments=(\d{4}) paths=(\d{4}) loops=(\d{4}) exits=(\d{4}) saddles=(\d{4})',lines[0])
    assert match, lines
    topology=list(map(int,match.groups()));segments=[];paths=[];coordinates={}
    for line in lines[1:]:
        if line.startswith('SEG '):
            match=re.fullmatch(r'SEG (\d{4}) (\d{4}) (\d{4})',line);assert match,line
            cell,a,b=map(int,match.groups());segments.append([cell,*sorted((a,b))])
        else:
            match=re.fullmatch(r'PATH (\d{4}) (open|closed)((?: \d{4}:\d{4},\d{4})+)',line);assert match,line
            assert int(match[1])==len(paths),line
            vertices=[]
            for edge,x,y in re.findall(r'(\d{4}):(\d{4}),(\d{4})',match[3]):
                point=[int(x),int(y)]
                key=str(int(edge));assert key not in coordinates or coordinates[key]==point
                coordinates[key]=point;vertices.append(int(edge))
            assert (vertices[0]==vertices[-1])==(match[2]=='closed')
            paths.append(canonical(vertices))
    return dict(segments=sorted(segments),paths=sorted(paths),coordinates=coordinates,topology=topology)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--executable',required=True)
    parser.add_argument('--output',default=str(ROOT/'build/validation'))
    arguments=parser.parse_args();output=Path(arguments.output);output.mkdir(parents=True,exist_ok=True)
    frozen=json.loads((ROOT/'data/FROZEN.json').read_text())
    for name,digest in frozen['files'].items():
        assert hashlib.sha256((ROOT/'data'/name).read_bytes()).hexdigest()==digest, f'Frozen input changed: {name}'
    fixtures=json.loads((ROOT/'data/fixtures.json').read_text());expected=json.loads((ROOT/'data/expected.json').read_text())
    results=[]
    for name,record in fixtures.items():
        result=subprocess.run([arguments.executable],input=record.encode(),capture_output=True,timeout=15)
        (output/f'{name}.stdout').write_bytes(result.stdout);(output/f'{name}.stderr').write_bytes(result.stderr)
        oracle=expected[name];assert result.returncode==oracle['exit'],(name,result.returncode,result.stderr.decode(errors='replace'))
        assert not result.stderr,(name,result.stderr)
        if oracle['exit']==64:assert result.stdout.decode()==oracle['stdout'],(name,result.stdout)
        else:
            observation=observe(result.stdout)
            assert observation=={k:v for k,v in oracle.items() if k!='exit'},(name,observation,oracle)
        results.append({'case':name,'exit':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest()})
    (output/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(f'Validated {len(results)} frozen terrain cases')
if __name__=='__main__':main()
