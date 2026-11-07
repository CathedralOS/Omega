"""Validate one compiled native artifact; never edits or regenerates the oracle."""
import argparse, hashlib, json, subprocess
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('executable',type=Path)
parser.add_argument('--output',type=Path,default=Path('build/validation'))
args=parser.parse_args()
root=Path(__file__).resolve().parent
args.output.mkdir(parents=True,exist_ok=True)
records=[]
for mode in range(1,7):
    payload=(root/'data'/f'input_{mode}.txt').read_bytes()
    expected=(root/'data'/f'expected_{mode}.txt').read_bytes()
    result=subprocess.run([str(args.executable.resolve())],input=payload,capture_output=True,timeout=30)
    (args.output/f'case_{mode}.stdout').write_bytes(result.stdout)
    (args.output/f'case_{mode}.stderr').write_bytes(result.stderr)
    assert result.returncode==0, (mode,result.returncode,result.stderr)
    assert result.stdout==expected, (mode,result.stdout,expected)
    assert result.stderr==b'', (mode,result.stderr)
    records.append(dict(case=mode,exit=result.returncode,stdout_sha256=hashlib.sha256(result.stdout).hexdigest()))
for name,payload in [('width',b'7\n'),('class',b'8\n'),('extent',b'9\n'),('unknown',b'0\n'),
                     ('text',b'x\n'),('extra',b'12\n'),('eof',b''),('unterminated',b'1'),('full',b'1234\n')]:
    result=subprocess.run([str(args.executable.resolve())],input=payload,capture_output=True,timeout=30)
    assert result.returncode==64, (name,result.returncode,result.stdout,result.stderr)
    assert result.stdout==b'LANDCOVER exact planar area; enter fixture 1..6 (7..9 invalid)\nINVALID: bounded fixture input or model admission\n'
    assert result.stderr==b''
    (args.output/f'invalid_{name}.stdout').write_bytes(result.stdout)
    records.append(dict(case=name,exit=64))
(args.output/'results.json').write_text(json.dumps(records,indent=2)+'\n')
print('PASS: six exact native output oracles and nine invalid runtime inputs')
