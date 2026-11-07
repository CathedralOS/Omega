#!/usr/bin/env python3
"""Run sequential public CLI routes against the previously frozen exact oracle."""
import argparse,hashlib,json,os,re,subprocess
from pathlib import Path
from fractions import Fraction
parser=argparse.ArgumentParser(); parser.add_argument('--omega',required=True); parser.add_argument('--route',choices=['check','native','terminal','invalid'],required=True)
args=parser.parse_args(); root=Path(__file__).resolve().parent; output=root/'build'/args.route; output.mkdir(parents=True,exist_ok=True)
expected_file=root/'fixtures/expected.json'
assert hashlib.sha256(expected_file.read_bytes()).hexdigest()==(root/'fixtures/oracle.sha256').read_text().split()[0], 'frozen oracle was changed'
expected=json.loads(expected_file.read_text())
def run(command,data=b'',name='check', executable=None):
 invocation=[executable or args.omega,*command]
 result=subprocess.run(invocation,input=data,capture_output=True)
 (output/(name+'.stdout')).write_bytes(result.stdout); (output/(name+'.stderr')).write_bytes(result.stderr)
 (output/(name+'.json')).write_text(json.dumps({'command':invocation,'exit':result.returncode},indent=2)+'\n')
 return result
if args.route=='check':
 result=run(['--check','--target','linux_x86_64',str(root/'main.omg')]); print(result.stderr.decode(errors='replace')); assert result.returncode==0
elif args.route=='invalid':
 result=run(['--check',str(root/'controls/intended_invalid/main.omg')],name='intended_invalid')
 text=result.stderr.decode(errors='replace')+result.stdout.decode(errors='replace')
 assert result.returncode==1,(result.returncode,text)
 for fragment in (root/'controls/intended_invalid/expected.txt').read_text().splitlines(): assert fragment in text,(fragment,text)
 print('intended-invalid rejected for the authored bounds contract')
else:
 native_binary=None
 if args.route=='native':
  compiled=run(['--build-dir',str(output/'artifact'),'--target','linux_x86_64',str(root/'main.omg')],name='compile')
  assert compiled.returncode==0,compiled.stderr.decode()
  match=re.search(r'published native output to (.+)',compiled.stdout.decode())
  assert match,compiled.stdout.decode()
  native_binary=match.group(1).strip()
  (output/'binary.sha256').write_text(hashlib.sha256(Path(native_binary).read_bytes()).hexdigest()+'  '+native_binary+'\n')
 for name,oracle in expected.items():
  command=['run']+(['--both'] if args.route=='terminal' else [])+['--keep',str(root/'main.omg')]
  result=run([] if native_binary else command,(root/'fixtures'/(name+'.stdin')).read_bytes(),name,executable=native_binary)
  text=result.stdout.decode(); assert result.returncode==oracle['exit'],(name,result.returncode,result.stderr.decode())
  if oracle['exit']:
   assert text==oracle['stdout'],(name,text)
  else:
   lines=text.splitlines(); assert len(lines)==4,(name,text)
   header=dict(piece.split('=') for piece in lines[0].split())
   for key in ['N','n','duplicate_query','duplicate_annotations']: assert int(header[key])==oracle[key],(name,key,header)
   for line,wanted in zip(lines[1:],oracle['results']):
    observed=dict(piece.split('=') for piece in line.split())
    assert int(observed['set'])==wanted['identifier']
    for key in ['K','hits','rank']: assert int(observed[key])==wanted[key],(name,key,line)
    for key in ['members','overlap']: assert observed[key]==wanted[key],(name,key,line)
    for key in ['p','q']: assert Fraction(observed[key])==Fraction(wanted[key]),(name,key,line)
  if args.route=='terminal':
   diagnostics=result.stderr.decode()
   assert 'DECLINED' not in diagnostics, (name,diagnostics)
   assert f"interp exit: {oracle['exit']}" in diagnostics and 'DIVERGENCE' not in diagnostics,(name,'missing explicit Terminal agreement',diagnostics)
  print(name,'verified',oracle['exit'])
print(args.route,'complete')
