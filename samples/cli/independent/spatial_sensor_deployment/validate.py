"""Sequential public-entrypoint validation against pre-execution frozen output."""
import argparse,json,pathlib,subprocess,time
p=argparse.ArgumentParser();p.add_argument('--omega',required=True);a=p.parse_args()
root=pathlib.Path(__file__).resolve().parent
out=root/'build/validation';out.mkdir(parents=True,exist_ok=True)
records=[]
def run(name,command,data=b''):
 start=time.monotonic()
 r=subprocess.run(command,input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=1200)
 (out/(name+'.stdout')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr)
 records.append(dict(name=name,command=command,input_hex=data.hex(),exit=r.returncode,elapsed_seconds=time.monotonic()-start))
 (out/'results.json').write_text(json.dumps(records,indent=2)+'\n')
 return r
r=run('check',[a.omega,'--check','--target','linux_x86_64',str(root/'main.omg')])
assert r.returncode==0,r.stderr.decode()
def verify(r,expected,exit_code,mode,name):
 assert r.returncode==exit_code,(mode,name,r.stderr.decode())
 assert r.stdout==expected,(mode,name,r.stdout.decode())
 if mode=='both':
  assert f'interp exit: {exit_code}'.encode() in r.stderr and b'DIVERGENCE' not in r.stderr and b'DECLINED' not in r.stderr,(mode,name,'Terminal agreement missing',r.stderr.decode())
for mode in ('native','both'):
 for n in range(8):
  command=[a.omega,'run']+(['--both'] if mode=='both' else [])+['--keep',str(root/'main.omg')]
  r=run(f'{mode}-{n}',command,str(n).encode())
  expected=(root/f'data/expected-{n}.txt').read_bytes()
  verify(r,expected,0,mode,n)
 for name,data in [('bad-letter',b'x'),('bad-digit',b'8'),('bad-newline',b'\n')]:
  r=run(mode+'-'+name,[a.omega,'run']+(['--both'] if mode=='both' else [])+['--keep',str(root/'main.omg')],data)
  verify(r,b'invalid command: expected one byte 0..7 or EOF\n',64,mode,name)
r=run('eof',[a.omega,'run','--both','--keep',str(root/'main.omg')])
verify(r,(root/'data/expected-0.txt').read_bytes(),0,'both','eof')
r=run('intended-invalid',[a.omega,'--check',str(root/'controls/invalid/invalid.omg')])
expected_error=(root/'controls/invalid/expected.txt').read_bytes().strip()
expected_diagnostic=b'cannot check prepared project: checked compilation failed for package `invalid_sensor_loan` with 1 diagnostic(s)\n  error: '+expected_error+b'\n'
assert r.returncode==1 and r.stdout==b'' and r.stderr==expected_diagnostic,r.stderr.decode()
print('PASS: check, 8 native + 8 Terminal exit-agreement cases, invalid runtime inputs, EOF, exact borrow rejection')
