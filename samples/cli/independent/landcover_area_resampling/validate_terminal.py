"""Sequential public run --both checks against frozen byte oracles."""
import argparse, hashlib, json, subprocess
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('omega',type=Path)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
root=Path(__file__).resolve().parent
args.output.mkdir(parents=True,exist_ok=True)
records=[]
cases=[(str(mode),(root/'data'/f'input_{mode}.txt').read_bytes(),0,
        (root/'data'/f'expected_{mode}.txt').read_bytes()) for mode in range(1,7)]
invalid=b'LANDCOVER exact planar area; enter fixture 1..6 (7..9 invalid)\nINVALID: bounded fixture input or model admission\n'
cases += [(name,payload,64,invalid) for name,payload in [
    ('width',b'7\n'),('class',b'8\n'),('extent',b'9\n'),('unknown',b'0\n'),
    ('text',b'x\n'),('extra',b'12\n'),('eof',b''),('unterminated',b'1'),('full',b'1234\n')]]
command=[str(args.omega.resolve()),'run','--both','--keep','--target','linux_x86_64',str(root/'main.omg')]
for name,payload,status,expected in cases:
    result=subprocess.run(command,input=payload,capture_output=True,timeout=300)
    (args.output/f'{name}.stdout').write_bytes(result.stdout)
    (args.output/f'{name}.stderr').write_bytes(result.stderr)
    assert result.returncode==status, (name,result.returncode,result.stderr)
    assert result.stdout==expected, (name,result.stdout,expected)
    assert f'interp exit: {status}'.encode() in result.stderr, (name,result.stderr)
    assert b'DECLINED' not in result.stderr and b'DIVERGENCE' not in result.stderr
    records.append(dict(case=name,exit=status,stdout_sha256=hashlib.sha256(result.stdout).hexdigest(),terminal_agrees=True,command=command))
    print(f'PASS: native and Terminal case {name}, exit {status}',flush=True)
(args.output/'results.json').write_text(json.dumps(records,indent=2)+'\n')
