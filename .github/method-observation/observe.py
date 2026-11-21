from pathlib import Path
import os, shutil, subprocess, tempfile, sys
root=Path.cwd(); cli=root/'target/debug/omega'
env=dict(os.environ,OMEGA_LIBRARY_ROOT=str(root/'source/library'))
base=root/'samples/cli/facts/method_array_operand_inference'
mode=sys.argv[1]
places=('local','const_name') if mode=='before' else ('local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body')
observations=[]
for place in places:
 for case in (place,'invalid_'+place):
  with tempfile.TemporaryDirectory() as p:
   path=Path(p)
   for f in ('main.omg','build.omg'):shutil.copyfile(base/case/f,path/f)
   r=subprocess.run([cli,'run','--both','--build-input','build.omg','--build-input','main.omg','build.omg'],cwd=path,env=env,stdin=subprocess.DEVNULL,capture_output=True,text=True)
   message=r.stderr.strip()
   print('CASE',case,r.returncode,message[:1000].replace('\n',' | '),flush=True)
   if case==place: passed=r.returncode==0 and 'native exit 0' in message and 'interp exit: 0' in message
   else: passed=r.returncode!=0 and ('expects' in message and '4' in message and '3' in message)
   observations.append((case,passed))
print('observations:', observations,flush=True)
if mode!='before':assert all(ok for _,ok in observations)
