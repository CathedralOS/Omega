from pathlib import Path
import os, shutil, subprocess, tempfile, sys
root=Path.cwd(); cli=root/'target/debug/omega'
env=dict(os.environ,OMEGA_LIBRARY_ROOT=str(root/'source/library'))
base=root/'samples/cli/facts/method_array_operand_inference'
mode=sys.argv[1]
places=('local','const_name') if mode=='before' else ('local',)
observations=[]
def measure_case(count):
 with tempfile.TemporaryDirectory() as tmp:
  dst=Path(tmp)
  build=(base/'local/build.omg').read_text().splitlines()
  (dst/'build.omg').write_text('\n'.join(line for line in build if 'select_provider' not in line)+'\n')
  (dst/'main.omg').write_text("""data Buffer<Element, const N: u64> { items: [Element; N]; }
machine measure<Element, const N: u64>(value: @Buffer<Element,N>) -> u64 { value.items.len }
machine main() crashes Trap {
    let bytes: [u8; 4] = [3, 5, 7, 11];
    let packet: Buffer<u8, 4> = Buffer { items: bytes };
    let observed: u64 = measure::<u8, COUNT>(@packet);
    transition observed == 4 { true -> done(), _ -> bad() }
    machine done() {} machine bad() { crash Trap; }
}
""".replace('COUNT', str(count)))
  r=subprocess.run([cli,'run','--both','--build-input','build.omg','--build-input','main.omg','build.omg'],cwd=dst,env=env,capture_output=True,text=True)
  msg=r.stderr.strip()
  print('MEASURE',count,r.returncode,msg[:2500].replace('\n',' | '),flush=True)
  return (r.returncode==0 and 'native exit 0' in msg and 'interp exit: 0' in msg) if count==4 else (r.returncode!=0 and 'unsupported' not in msg and ('array' in msg or 'const' in msg))

def borrowed_case(invalid):
 with tempfile.TemporaryDirectory() as tmp:
  dst=Path(tmp);shutil.copyfile(base/'local/build.omg',dst/'build.omg')
  declarations=(base/'local/main.omg').read_text().split('machine main()')[0]
  source=declarations+"data Ticket [copy] { number: u64; }\nmachine pick<'a>(first: @'a Ticket, second: @'a Ticket, third: @'a Ticket) -> @'a Ticket {\n    let values: [@'a Ticket; 3] = [first, second, third];\n    let selected: @'a Ticket = Bytes::last::<3u64, @'a Ticket>(@values);\n    selected\n}\nmachine main() crashes Trap {\n    let one: Ticket = Ticket { number: 7 };\n    let two: Ticket = Ticket { number: 11 };\n    let mut three: Ticket = Ticket { number: 13 };\n    let observed: @Ticket = pick(@one, @two, @three);\n    MUTATION\n    transition observed.number == 13 { true -> done(), _ -> bad() }\n    machine done() {} machine bad() { crash Trap; }\n}\n"
  (dst/'main.omg').write_text(source.replace('MUTATION','three.number = 21;' if invalid else ''))
  r=subprocess.run([cli,'run','--both','--build-input','build.omg','--build-input','main.omg','build.omg'],cwd=dst,env=env,capture_output=True,text=True)
  msg=r.stderr.strip()
  print('BORROW',invalid,r.returncode,msg[:2500].replace('\n',' | '),flush=True)
  return (r.returncode!=0 and 'not supported yet' not in msg and ('loan' in msg or 'borrow' in msg)) if invalid else (r.returncode==0 and 'native exit 0' in msg and 'interp exit: 0' in msg)
for place in places:
 for case in ((place,) if place in ('.','element_not_copy','declarations') else (place,'invalid_'+place)):
  with tempfile.TemporaryDirectory() as p:
   path=Path(p)
   control=case=='element_not_copy'
   source=base/('local' if control or case=='declarations' else case)
   for f in ('main.omg','build.omg'):shutil.copyfile(source/f,path/f)
   if case=='declarations':
    f=path/'main.omg';f.write_text(f.read_text().split('machine main()')[0]+'machine main() {}\n')
    f=path/'build.omg';f.write_text('\n'.join(l for l in f.read_text().splitlines() if 'select_provider' not in l)+'\n')
   if control:
    text=(path/'main.omg').read_text().replace('boundary trait Bytes', 'data Token { value: u8; }\nboundary trait Bytes').replace('let original: [u8; 3] = [7, 11, 13];','let bytes: [Token; 3] = [Token { value: 7 }, Token { value: 11 }, Token { value: 13 }];').replace('    let bytes: [u8; 3] = original;\n','').replace('let observed: u8', 'let observed: Token').replace('transition observed ==', 'transition observed.value ==')
    (path/'main.omg').write_text(text)
   r=subprocess.run([cli,'run','--both','--build-input','build.omg','--build-input','main.omg','build.omg'],cwd=path,env=env,stdin=subprocess.DEVNULL,capture_output=True,text=True)
   message='\n'.join(dict.fromkeys(r.stderr.strip().splitlines()))
   print('CASE',case,r.returncode,message[:5000].replace('\n',' | '),flush=True)
   if control: passed=r.returncode!=0 and 'binder requires' in message and 'copy' in message
   elif case==place: passed=r.returncode==0 and 'native exit 0' in message and 'interp exit: 0' in message
   else: passed=r.returncode!=0 and 'expects an array with 4 elements but the supplied array has 3' in message
   observations.append((case,passed))

print('observations:', observations,flush=True)
if mode!='before':assert all(ok for _,ok in observations)
