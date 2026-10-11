"""Sequential public compiler/application acceptance; no parallel executor jobs."""
import argparse,json,os,subprocess,time,hashlib
from pathlib import Path
from validate import verify
ROOT=Path(__file__).resolve().parent
REPOSITORY=ROOT.parents[3]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--omega',required=True);arguments=parser.parse_args()
    binary=Path(arguments.omega).resolve();build=ROOT/'build';build.mkdir(exist_ok=True)
    environment=os.environ.copy();environment['OMEGA_LIBRARY_ROOT']=str(REPOSITORY/'source'/'library')
    receipts=[]
    def run(label,command,input_path=None):
        started=time.monotonic();result=subprocess.run(command,input=input_path.read_bytes() if input_path else None,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=environment,cwd=REPOSITORY,timeout=300)
        (build/f'{label}.stdout').write_bytes(result.stdout);(build/f'{label}.stderr').write_bytes(result.stderr)
        receipts.append({'label':label,'command':list(map(str,command)),'input':str(input_path) if input_path else None,'exit':result.returncode,'seconds':time.monotonic()-started})
        (build/'receipts.json').write_text(json.dumps(receipts,indent=2)+'\n')
        print(label,result.returncode,flush=True);return result
    result=run('check',[str(binary),'--check','--target','linux_x86_64',str(ROOT/'main.omg')]);assert result.returncode==0,result.stderr.decode()
    result=run('invalid_source',[str(binary),'--check',str(ROOT/'invalid'/'main.omg')]);assert result.returncode==1 and b'count' in result.stderr and (b'prove' in result.stderr or b'constraint' in result.stderr),result.stderr.decode()
    result=run('compile',[str(binary),'--target','linux_x86_64',str(ROOT/'main.omg')]);assert result.returncode==0,result.stderr.decode()
    executables=[p for p in build.rglob('*') if p.is_file() and os.access(p,os.X_OK) and not p.name.endswith('.py')]
    assert len(executables)==1,executables
    executable=executables[0]
    for fixture in sorted((ROOT/'data').glob('*.txt')):
        name=fixture.stem;result=run(f'native_{name}',[str(executable)],fixture)
        if name.startswith('invalid_'):assert result.returncode==64 and result.stdout==b'INVALID_INPUT\n'
        else:assert result.returncode==0;verify(name,build/f'native_{name}.stdout')
    for fixture in sorted((ROOT/'data').glob('*.txt')):
        name=fixture.stem;result=run(f'terminal_{name}',[str(binary),'run','--both',str(ROOT/'main.omg')],fixture)
        assert b'AGREEMENT' in result.stderr and b'DECLINED' not in result.stderr,result.stderr.decode()
        if name.startswith('invalid_'):assert result.returncode==64 and result.stdout==b'INVALID_INPUT\n'
        else:assert result.returncode==0;verify(name,build/f'terminal_{name}.stdout')
    print('ALL ACCEPTANCE CHECKS PASS',flush=True)
if __name__=='__main__':main()
