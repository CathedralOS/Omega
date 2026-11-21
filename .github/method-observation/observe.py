from pathlib import Path
import os, shutil, subprocess, tempfile, sys
root=Path.cwd(); cli=root/'target/debug/omega'
env=dict(os.environ,OMEGA_LIBRARY_ROOT=str(root/'source/library'))
base=root/'samples/cli/facts/method_array_operand_inference'
mode=sys.argv[1]
places=('local','const_name') if mode=='before' else ('local','target_body','element_not_copy')
observations=[]
for place in places:
 for case in ((place,) if place in ('.','element_not_copy') else (place,'invalid_'+place)):
  with tempfile.TemporaryDirectory() as p:
   path=Path(p)
   control=case=='element_not_copy'
   source=base/('local' if control else case)
   for f in ('main.omg','build.omg'):shutil.copyfile(source/f,path/f)
   if control:
    text=(path/'main.omg').read_text().replace('boundary trait Bytes', 'data Token { value: u8; }\nboundary trait Bytes').replace('let original: [u8; 3] = [7, 11, 13];','let bytes: [Token; 3] = [Token { value: 7 }, Token { value: 11 }, Token { value: 13 }];').replace('    let bytes: [u8; 3] = original;\n','').replace('let observed: u8', 'let observed: Token').replace('transition observed ==', 'transition observed.value ==')
    (path/'main.omg').write_text(text)
   r=subprocess.run([cli,'run','--both','--build-input','build.omg','--build-input','main.omg','build.omg'],cwd=path,env=env,stdin=subprocess.DEVNULL,capture_output=True,text=True)
   message=r.stderr.strip()
   print('CASE',case,r.returncode,message[:5000].replace('\n',' | '),flush=True)
   if control: passed=r.returncode!=0 and 'binder requires' in message and 'copy' in message
   elif case==place: passed=r.returncode==0 and 'native exit 0' in message and 'interp exit: 0' in message
   else: passed=r.returncode!=0 and ('expects' in message and '4' in message and '3' in message)
   observations.append((case,passed))
print('observations:', observations,flush=True)
if mode!='before':assert all(ok for _,ok in observations)
