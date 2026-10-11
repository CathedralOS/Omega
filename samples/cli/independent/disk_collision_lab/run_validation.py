"""Serial native/Terminal validation with retained artifacts and exact statuses.
Requires prior authorized package review; never changes package acceptance.
Usage: python3 run_validation.py /absolute/compiler [native-binary]
"""
import hashlib,json,os,re,shutil,subprocess,sys
sys.dont_write_bytecode=True
from pathlib import Path
from validate import validate
root=Path(__file__).resolve().parent
repository=root.parents[3]
out=root/'build/validation';out.mkdir(parents=True,exist_ok=True)
compiler=Path(sys.argv[1]).resolve()
environment=os.environ.copy();environment['OMEGA_LIBRARY_ROOT']=str(repository/'source/library')
records=[]
def run(name,command,input_path=None):
    result=subprocess.run(command,input=input_path.read_bytes() if input_path else None,capture_output=True,env=environment)
    (out/(name+'.stdout')).write_bytes(result.stdout)
    (out/(name+'.stderr')).write_bytes(result.stderr)
    records.append(dict(name=name,command=[str(c) for c in command],input=str(input_path) if input_path else None,exit=result.returncode))
    (out/'commands.json').write_text(json.dumps(records,indent=2)+'\n')
    return result
# This file never authorizes update or package review.
check=run('check',[compiler,'--check','--target','linux_x86_64',root/'main.omg'])
assert check.returncode==0,check.stderr.decode(errors='replace')
control_root=out/'intended_invalid'
control_root.mkdir(exist_ok=True)
for name in ('main.omg','build.omg'):
    shutil.copyfile(root/'controls/intended_invalid'/name,control_root/name)
control=run('invalid_control',[compiler,'--check','--target','linux_x86_64',control_root/'main.omg'])
assert control.returncode==1,control.stderr.decode(errors='replace')
assert b'cannot prove index' in control.stdout+control.stderr,control.stderr.decode(errors='replace')
if len(sys.argv)>2:
    native=Path(sys.argv[2]).resolve()
else:
    compile_result=run('compile',[compiler,'--target','linux_x86_64',root/'main.omg'])
    assert compile_result.returncode==0,compile_result.stderr.decode(errors='replace')
    published=re.findall(rb'^published native output to (.+)$',compile_result.stdout,re.MULTILINE)
    assert len(published)==1,compile_result.stdout
    native=Path(os.fsdecode(published[0])).resolve()
    assert native.is_file() and os.access(native,os.X_OK),native
for case in range(10):
    fixture=root/f'data/case_{case}.stdin'
    result=run(f'native_{case}',[native],fixture)
    validate(str(case),out/f'native_{case}.stdout',result.returncode)
for fixture in sorted((root/'data').glob('invalid_*.stdin')):
    result=run('native_'+fixture.stem,[native],fixture)
    assert result.returncode==2,(fixture,result.returncode)
    assert result.stdout==b'invalid input: one digit 0..9 and LF required\n',(fixture,result.stdout)
for case in range(10):
    result=run(f'both_{case}',[compiler,'run','--both',root/'main.omg'],root/f'data/case_{case}.stdin')
    validate(str(case),out/f'both_{case}.stdout',result.returncode)
    assert b'DECLINED' not in result.stderr,result.stderr.decode(errors='replace')
    assert ('interp exit: '+str(result.returncode)).encode() in result.stderr,result.stderr.decode(errors='replace')
for fixture in sorted((root/'data').glob('invalid_*.stdin')):
    result=run('both_'+fixture.stem,[compiler,'run','--both',root/'main.omg'],fixture)
    assert result.returncode==2,(fixture,result.returncode)
    assert result.stdout==b'invalid input: one digit 0..9 and LF required\n',(fixture,result.stdout)
    assert b'DECLINED' not in result.stderr,result.stderr.decode(errors='replace')
    assert b'interp exit: 2' in result.stderr,result.stderr.decode(errors='replace')
print('validated native fixtures, invalid inputs, bounds control, and Terminal comparison; '+str(out))
